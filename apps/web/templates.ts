import type { Json } from "@perishlab/radicula-document";
import { Failure, request } from "./client.ts";

export type Brief = {
	number: number;
	document: number;
	revision: string;
};

export type Meta = {
	name: string;
	builtin: boolean;
};

export type Template<T = number> = Meta & {
	latest: number;
	editions: T[];
};

export type Templates = {
	templates: Template[];
};

export type Edition = {
	template: string;
	number: number;
	document: number;
	revision: string;
	schema: Record<string, Json>;
};

export type Fault = {
	id: string;
	edition: number;
	reason: string;
};

export type Status = {
	template: string;
	latest: number;
	format: string;
	ready: boolean;
	checked: number;
	compatible: number;
	incompatible: Fault[];
};

export type Options = {
	access: () => string | undefined;
	fetcher?: typeof fetch;
};

const pattern = /^[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?$/;

export function valid(slug: string): boolean {
	return pattern.test(slug);
}

export function parse(text: string): Record<string, Json> {
	let parsed: unknown;
	try {
		parsed = JSON.parse(text);
	} catch {
		throw new Failure(422, "模式必须是合法的 JSON 对象。");
	}
	if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
		throw new Failure(422, "模式根节点必须是 JSON 对象。");
	}
	return parsed as Record<string, Json>;
}

export function list(options: Options): Promise<Templates> {
	return request<Templates>("/api/v1/templates", options);
}

export function detail(
	target: { name: string },
	options: Options,
): Promise<Template<Brief>> {
	return request<Template<Brief>>(
		`/api/v1/templates/${encodeURIComponent(target.name)}`,
		options,
	);
}

export function edition(
	target: { name: string; number: number },
	options: Options,
): Promise<Edition> {
	return request<Edition>(
		`/api/v1/templates/${encodeURIComponent(target.name)}/editions/${target.number}`,
		options,
	);
}

export function status(
	target: { name: string },
	options: Options,
): Promise<Status> {
	return request<Status>(
		`/api/v1/templates/${encodeURIComponent(target.name)}/status`,
		options,
	);
}

export function create(
	input: { name: string; schema: Record<string, Json> },
	options: Options,
): Promise<Edition> {
	const name = input.name.trim();
	if (!valid(name) || name === "meta") {
		return Promise.reject(
			new Failure(422, "模板名称必须为小写字母、数字或连字符，且不能为 meta。"),
		);
	}
	return request<Edition>("/api/v1/templates", {
		...options,
		method: "POST",
		body: { name, schema: input.schema },
	});
}

export function publish(
	input: { name: string; base: number; schema: Record<string, Json> },
	options: Options,
): Promise<Edition> {
	if (input.name === "meta") {
		return Promise.reject(new Failure(403, "内建 meta 模板不可发布新版本。"));
	}
	return request<Edition>(
		`/api/v1/templates/${encodeURIComponent(input.name)}/editions`,
		{
			...options,
			method: "POST",
			body: { base: input.base, schema: input.schema },
		},
	);
}
