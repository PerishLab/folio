import { context, locus, safe, stored, tally } from "./observe.ts";
import { type Config, configuration, Refusal } from "./src/lib/discovery.ts";
import { claim, exchange } from "./src/lib/token.ts";
import { clear, Locker, type Vault } from "./src/lib/vault.ts";

export { type Config, configuration, Refusal, type Vault };

type Seat = {
	origin: string;
	back: string;
	storage: Vault;
	fallback?: Vault;
	crypto: Crypto;
};
type Callback = {
	url: string;
	storage: Vault;
	fallback?: Vault;
	fetcher: typeof fetch;
};
type Journey = Config & {
	origin: string;
	back: string;
	state: string;
	verifier: string;
};
type Prepared = { journey: Journey; url: URL; code: string };

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
	clear();
	if (had) notify();
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

function inspect(href: string): { query?: URLSearchParams; state?: string } {
	try {
		const query = new URL(href).searchParams;
		return { query, state: single(query, "state") };
	} catch {
		return {};
	}
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
		const saved = new Locker(seat.storage, seat.fallback).save(
			state,
			JSON.stringify(journey),
		);
		stored(saved.verified, saved.keys, saved.backed);
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

function prepare(
	config: Config,
	locker: Locker,
	source: string | undefined,
	href: string,
): Prepared {
	if (!source) {
		locus.warn("callback storage missing");
		tally("storage.missing");
		throw new Refusal("登录上下文已失效（本地会话缺失），请重新登录。");
	}
	const journey = parse(source);
	locker.take(journey.state);
	const url = new URL(href);
	match(journey, config, url);
	const code = verify(url.searchParams, journey.state);
	return { journey, url, code };
}

export function complete(config: Config, seat: Callback): Promise<string> {
	return locus.trace("auth.complete", async () => {
		const locker = new Locker(seat.storage, seat.fallback);
		const keys = locker.count();
		const raw = locker.pull();
		access = undefined;
		notify();
		const parsed = inspect(seat.url);
		const backup = raw ? undefined : locker.take(parsed.state);
		const recovered = !raw && Boolean(backup);
		const diag = context(parsed.query, Boolean(raw), recovered, keys);
		const ready = locus.with(diag, () => {
			if (recovered) {
				locus.warn(
					"callback session storage missing; recovered via local fallback",
				);
			}
			return prepare(config, locker, raw ?? backup, seat.url);
		});
		const form = new URLSearchParams({
			grant_type: "authorization_code",
			client_id: config.client,
			redirect_uri: `${ready.url.origin}/callback`,
			code: ready.code,
			code_verifier: ready.journey.verifier,
		});
		const response = await exchange(config.token, seat.fetcher, form, diag);
		const body = await response.json();
		return locus.with(diag, () => {
			access = claim(body);
			notify();
			locus.info(
				recovered ? "callback completed via fallback" : "callback completed",
			);
			tally(recovered ? "ok.fallback" : "ok");
			return ready.journey.back;
		});
	})();
}
