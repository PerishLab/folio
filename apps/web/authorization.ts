export class Refusal extends Error {}

export type Config = {
	issuer: string;
	client: string;
	audience: string;
	authorize: string;
	token: string;
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
	return {
		issuer,
		client,
		audience,
		authorize,
		token: endpoint ? `${base}/token` : token,
	};
}

export async function start(config: Config, seat: Seat): Promise<string> {
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
}

function match(journey: Journey, config: Config, url: URL): void {
	const check = [
		journey.origin === url.origin,
		journey.issuer === config.issuer,
		journey.client === config.client,
		journey.audience === config.audience,
		url.pathname === "/callback",
		valid(journey.back),
	];
	if (check.some((ok) => !ok)) {
		throw new Refusal("身份确认不匹配，请重新登录。");
	}
}

function claim(result: Record<string, unknown>): {
	value: string;
	until: number;
} {
	const token = text(result.access_token);
	const seconds = result.expires_in;
	if (result.token_type !== "Bearer" || !token) {
		throw new Refusal("暂时无法完成身份确认。");
	}
	if (
		typeof seconds !== "number" ||
		!Number.isFinite(seconds) ||
		seconds <= 0
	) {
		throw new Refusal("暂时无法完成身份确认。");
	}
	return { value: token, until: Date.now() + seconds * 1000 };
}

export async function complete(
	config: Config,
	seat: { url: string; storage: Vault; fetcher: typeof fetch },
): Promise<string> {
	const raw = seat.storage.getItem(slot);
	seat.storage.removeItem(slot);
	access = undefined;
	notify();
	if (!raw) throw new Refusal("身份确认已失效，请重新登录。");
	const journey: Journey = JSON.parse(raw);
	const url = new URL(seat.url);
	match(journey, config, url);
	const query = url.searchParams;
	const code = single(query, "code");
	if (single(query, "state") !== journey.state || !code || query.has("error")) {
		throw new Refusal("身份确认不匹配，请重新登录。");
	}
	const fetcher = seat.fetcher;
	const response = await fetcher(config.token, {
		method: "POST",
		cache: "no-store",
		credentials: "omit",
		redirect: "error",
		signal: AbortSignal.timeout(15000),
		headers: { "content-type": "application/x-www-form-urlencoded" },
		body: new URLSearchParams({
			grant_type: "authorization_code",
			client_id: config.client,
			redirect_uri: `${url.origin}/callback`,
			code,
			code_verifier: journey.verifier,
		}),
	});
	if (!response.ok) throw new Refusal("身份确认已失效，请重新登录。");
	access = claim(await response.json());
	notify();
	return journey.back;
}
