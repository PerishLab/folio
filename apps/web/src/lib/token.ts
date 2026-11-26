import { locus, safe, tally } from "../../observe.ts";
import { Refusal, text } from "./discovery.ts";

async function reason(response: Response): Promise<string | undefined> {
	try {
		const body = await response.json();
		return safe(body?.error);
	} catch {
		return undefined;
	}
}

export async function exchange(
	endpoint: string,
	fetcher: typeof fetch,
	form: URLSearchParams,
	diag: Record<string, string>,
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
		locus.with(diag, () => {
			locus.fault(new Error("token exchange failed"), { stage: "exchange" });
			tally("token.failed", { status: "network" });
		});
		throw new Refusal("登录凭证交换失败，请重新登录。");
	}
	if (!response.ok) {
		const code = await reason(response);
		const detail: Record<string, string | number | boolean> = {
			status: response.status,
		};
		if (code) detail.reason = code;
		locus.with(diag, () => {
			locus.fault(new Error("token exchange failed"), detail);
			tally("token.failed", { status: String(response.status) });
		});
		throw new Refusal("登录凭证交换失败，请重新登录。");
	}
	return response;
}

export function claim(result: Record<string, unknown>): {
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
