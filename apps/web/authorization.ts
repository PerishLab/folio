import { activate, locus, safe, tally } from "./observe.ts";

export class Refusal extends Error {}

export type Config = {
	issuer: string;
	client: string;
	audience: string;
	authorize: string;
	token: string;
	observe?: string;
};

type Vault = Pick<Storage, "getItem" | "setItem" | "removeItem">;
type Seat = { origin: string; back: string; storage: Vault; crypto: Crypto };
type Journey = Config & {
	origin: string;
	back: string;
	state: string;
	verifier: string;
};

const slot = "folio.authorization";
const routes = ["/", "/tags", "/templates", "/documents"];
const listeners = new Set<() => void>();
let access: { value: string; until: number } | undefined;

function notify(): void {
	for (const listener of listeners) listener();
}

export function subscribe(listener: () => void): () => void {
	listeners.add(listener);
	return () => listeners.delete(listener);
}

export function held(): string | undefined {
	if (!access) return undefined;
	if (access.until > Date.now()) return access.value;
	access = undefined;
	return undefined;
}

export function forget(): void {
	const had = access !== undefined;
	access = undefined;
	globalThis.sessionStorage?.removeItem(slot);
	if (had) notify();
}

function trim(value: string): string {
	return value.replace(/\/+$/, "");
}

function encode(bytes: Uint8Array): string {
	return btoa(String.fromCharCode(...bytes))
		.replaceAll("+", "-")
		.replaceAll("/", "_")
		.replaceAll("=", "");
}

function valid(path: string): boolean {
	return routes.includes(path);
}

function single(query: URLSearchParams, key: string): string | undefined {
	const values = query.getAll(key);
	if (values.length !== 1) return undefined;
	return values[0] || undefined;
}

function text(value: unknown): string | undefined {
	return typeof value === "string" && value.length > 0 ? value : undefined;
}

export async function configuration(
	fetcher: typeof fetch = fetch,
): Promise<Config | undefined> {
	const response = await fetcher("/client.json", {
		cache: "no-store",
		credentials: "omit",
		redirect: "error",
		signal: AbortSignal.timeout(15000),
	});
	if (!response.ok) throw new Refusal("暂时无法读取客户端配置，请重试。");
	const raw = await response.json();
	if (typeof raw !== "object" || raw === null || Array.isArray(raw)) {
		throw new Refusal("暂时无法确认身份配置，请重试。");
	}
	const observe = text(raw.observe);
	if (observe) activate(observe);
	if (!raw.issuer && !raw.client) return undefined;
	const issuer = text(raw.issuer);
	const client = text(raw.client);
	if (!issuer || !client) throw new Refusal("暂时无法确认身份配置，请重试。");
	const endpoint = text(raw.endpoint);
	const audience = text(raw.audience) ?? "folio";
	const base = trim(endpoint ?? issuer);
	const discovery = await fetcher(`${base}/.well-known/openid-configuration`, {
		cache: "no-store",
		credentials: "omit",
		redirect: "error",
		signal: AbortSignal.timeout(15000),
	});
	if (!discovery.ok) throw new Refusal("暂时无法确认身份服务，请重试。");
	const known = await discovery.json();
	const authorize = text(known.authorization_endpoint);
	const token = text(known.token_endpoint);
	if (known.issuer !== issuer || !authorize || !token) {
		throw new Refusal("无法确认身份服务的有效签发方。");
	}
	const resolved = endpoint ? `${base}/token` : token;
	return observe
		? { issuer, client, audience, authorize, token: resolved, observe }
		: { issuer, client, audience, authorize, token: resolved };
}

export function start(config: Config, seat: Seat): Promise<string> {
	return locus.trace("auth.start", async () => {
		if (!valid(seat.back)) throw new Refusal("返回地址无效。");
		const verifier = encode(seat.crypto.getRandomValues(new Uint8Array(48)));
		const state = encode(seat.crypto.getRandomValues(new Uint8Array(32)));
		const hash = await seat.crypto.subtle.digest(
			"SHA-256",
			new TextEncoder().encode(verifier),
		);
		const journey: Journey = {
			...config,
			origin: seat.origin,
			back: seat.back,
			state,
			verifier,
		};
		seat.storage.setItem(slot, JSON.stringify(journey));
		const target = new URL(config.authorize, seat.origin);
		if (target.protocol !== "https:" && target.protocol !== "http:") {
			throw new Refusal("身份服务授权地址无效。");
		}
		target.search = new URLSearchParams({
			response_type: "code",
			client_id: config.client,
			redirect_uri: `${seat.origin}/callback`,
			scope: "openid profile",
			resource: config.audience,
			state,
			code_challenge: encode(new Uint8Array(hash)),
			code_challenge_method: "S256",
		}).toString();
		return target.href;
	})();
}

function parse(raw: string): Journey {
	try {
		return JSON.parse(raw) as Journey;
	} catch {
		locus.fault(new Error("callback storage parse failed"), { stage: "parse" });
		tally("parse.failed");
		throw new Refusal("登录上下文已失效，请重新登录。");
	}
}

function mismatch(
	journey: Journey,
	expected: Config,
	url: URL,
): string | undefined {
	if (journey.origin !== url.origin) return "origin";
	if (journey.issuer !== expected.issuer) return "issuer";
	if (journey.client !== expected.client) return "client";
	if (journey.audience !== expected.audience) return "audience";
	if (url.pathname !== "/callback") return "path";
	if (!valid(journey.back)) return "back";
	return undefined;
}

function match(journey: Journey, expected: Config, url: URL): void {
	const field = mismatch(journey, expected, url);
	if (!field) return;
	locus.warn(`callback match failed: ${field}`);
	tally("match.failed", { field });
	throw new Refusal("身份确认不匹配，请重新登录。");
}

function verify(query: URLSearchParams, state: string): string {
	if (query.has("error")) {
		const code = safe(single(query, "error")) ?? "unknown";
		locus.warn(`callback oauth error: ${code}`);
		tally("oauth.error", { reason: code });
		throw new Refusal("身份确认不匹配，请重新登录。");
	}
	if (single(query, "state") !== state) {
		locus.warn("callback state mismatch");
		tally("state.mismatch");
		throw new Refusal("身份确认不匹配，请重新登录。");
	}
	const code = single(query, "code");
	if (!code) {
		locus.warn("callback code missing");
		tally("code.missing");
		throw new Refusal("身份确认不匹配，请重新登录。");
	}
	return code;
}

async function reason(response: Response): Promise<string | undefined> {
	try {
		const body = await response.json();
		return safe(body?.error);
	} catch {
		return undefined;
	}
}

async function exchange(
	endpoint: string,
	fetcher: typeof fetch,
	form: URLSearchParams,
): Promise<Response> {
	let response: Response;
	try {
		response = await fetcher(endpoint, {
			method: "POST",
			cache: "no-store",
			credentials: "omit",
			redirect: "error",
			signal: AbortSignal.timeout(15000),
			headers: { "content-type": "application/x-www-form-urlencoded" },
			body: form,
		});
	} catch {
		locus.fault(new Error("token exchange failed"), { stage: "exchange" });
		tally("token.failed", { status: "network" });
		throw new Refusal("登录凭证交换失败，请重新登录。");
	}
	if (!response.ok) {
		const code = await reason(response);
		const detail: Record<string, string | number | boolean> = {
			status: response.status,
		};
		if (code) detail.reason = code;
		locus.fault(new Error("token exchange failed"), detail);
		tally("token.failed", { status: String(response.status) });
		throw new Refusal("登录凭证交换失败，请重新登录。");
	}
	return response;
}

function claim(result: Record<string, unknown>): {
	value: string;
	until: number;
} {
	const token = text(result.access_token);
	const seconds = result.expires_in;
	const oktype = result.token_type === "Bearer" && Boolean(token);
	const oktime =
		typeof seconds === "number" && Number.isFinite(seconds) && seconds > 0;
	if (!oktype || !oktime || !token) {
		locus.warn("callback claim invalid");
		tally("claim.invalid");
		throw new Refusal("暂时无法完成身份确认。");
	}
	return { value: token, until: Date.now() + seconds * 1000 };
}

export function complete(
	config: Config,
	seat: { url: string; storage: Vault; fetcher: typeof fetch },
): Promise<string> {
	return locus.trace("auth.complete", async () => {
		const raw = seat.storage.getItem(slot);
		seat.storage.removeItem(slot);
		access = undefined;
		notify();
		if (!raw) {
			locus.warn("callback storage missing");
			tally("storage.missing");
			throw new Refusal("登录上下文已失效（本地会话缺失），请重新登录。");
		}
		const journey = parse(raw);
		const url = new URL(seat.url);
		match(journey, config, url);
		const code = verify(url.searchParams, journey.state);
		const form = new URLSearchParams({
			grant_type: "authorization_code",
			client_id: config.client,
			redirect_uri: `${url.origin}/callback`,
			code,
			code_verifier: journey.verifier,
		});
		const response = await exchange(config.token, seat.fetcher, form);
		access = claim(await response.json());
		notify();
		locus.info("callback completed");
		tally("ok");
		return journey.back;
	})();
}
