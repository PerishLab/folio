import type { Reply } from "@perishlab/radicula-document";
import { forget } from "./authorization.ts";

export const uncertain =
	"保存结果尚未确认，已暂停继续写入；请先重新读取确认当前状态后再决定是否重新提交。";

const states: Record<number, "conflict" | "rejected" | undefined> = {
	400: "rejected",
	401: "rejected",
	403: "rejected",
	404: "rejected",
	409: "conflict",
	422: "rejected",
};

const messages: Record<number, string> = {
	400: "请求参数格式无效，请检查后重试。",
	401: "身份凭据已失效，请重新登录。",
	403: "当前身份没有执行此操作的权限。",
	404: "目标资源不存在或已被移除。",
	409: "数据已在其他会话更新，请刷新后重试。",
	422: "提交内容未通过校验，请检查字段与模式要求。",
	503: "服务暂时不可用，请稍后重试。",
};

export function describe(status: number, fallback?: string): string {
	if (fallback) return fallback;
	if (messages[status]) return messages[status];
	return status >= 500
		? "服务暂时不可用，请稍后重试。"
		: "暂时无法完成操作，请重试。";
}

export class Failure extends Error {
	status: number;
	code?: string;
	uncertain: boolean;

	constructor(status: number, text?: string, code?: string, doubt = false) {
		super(describe(status, text));
		this.status = status;
		this.code = code;
		this.uncertain = doubt;
	}
}

export type Options = {
	access: () => string | undefined;
	fetcher?: typeof fetch;
	method?: string;
	body?: unknown;
	empty?: boolean;
};

async function category(response: Response): Promise<string | undefined> {
	try {
		const payload = await response.json();
		return typeof payload?.error === "string" ? payload.error : undefined;
	} catch {
		return undefined;
	}
}

export async function request<T>(path: string, options: Options): Promise<T> {
	const token = options.access();
	if (!token) throw new Failure(401);
	const fetcher = options.fetcher ?? fetch;
	const method = options.method ?? "GET";
	const write = method !== "GET";
	const headers: Record<string, string> = {
		authorization: `Bearer ${token}`,
	};
	const body =
		options.body === undefined ? undefined : JSON.stringify(options.body);
	if (body !== undefined) headers["content-type"] = "application/json";
	let response: Response;
	try {
		response = await fetcher(path, {
			method,
			cache: "no-store",
			credentials: "omit",
			redirect: "error",
			signal: AbortSignal.timeout(15000),
			headers,
			body,
		});
	} catch {
		if (write) throw new Failure(0, uncertain, "uncertain", true);
		throw new Failure(0, "网络请求中断，请重试。");
	}
	if (response.status === 401) {
		forget();
		throw new Failure(401, undefined, await category(response));
	}
	if (!response.ok) {
		const kind = await category(response);
		const doubt = write && (response.status === 429 || response.status >= 500);
		throw new Failure(
			response.status,
			doubt ? uncertain : undefined,
			kind,
			doubt,
		);
	}
	if (options.empty || response.status === 204) return undefined as T;
	return (await response.json()) as T;
}

export async function reply(
	response: Response,
	conflict: string,
): Promise<Reply> {
	if (response.ok) {
		return { state: "saved", snapshot: await response.json() };
	}
	const code = response.status;
	if (code === 401) forget();
	const state: "conflict" | "rejected" | "unknown" = states[code] ?? "unknown";
	const note =
		state === "conflict"
			? conflict
			: state === "unknown"
				? uncertain
				: undefined;
	return {
		state,
		error: new Failure(code, note, undefined, state === "unknown"),
	};
}
