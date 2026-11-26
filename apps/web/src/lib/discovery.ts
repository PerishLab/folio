import { activate, endpoint } from "../../observe.ts";

export class Refusal extends Error {}

export type Config = {
	issuer: string;
	client: string;
	audience: string;
	authorize: string;
	token: string;
	observe?: string;
};

function trim(value: string): string {
	return value.replace(/\/+$/, "");
}

export function text(value: unknown): string | undefined {
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
	const observe = endpoint(raw.observe);
	activate(observe);
	if (!raw.issuer && !raw.client) return undefined;
	const issuer = text(raw.issuer);
	const client = text(raw.client);
	if (!issuer || !client) throw new Refusal("暂时无法确认身份配置，请重试。");
	const override = text(raw.endpoint);
	const audience = text(raw.audience) ?? "folio";
	const base = trim(override ?? issuer);
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
	const resolved = override ? `${base}/token` : token;
	return observe
		? { issuer, client, audience, authorize, token: resolved, observe }
		: { issuer, client, audience, authorize, token: resolved };
}
