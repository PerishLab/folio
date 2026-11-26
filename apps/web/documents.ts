import type { Json } from "@perishlab/radicula-document";
import { Failure, request } from "./client.ts";

export type Listed = {
	id: string;
	template: string;
	edition: number;
	revision: string;
};

export type Listing = {
	documents: Listed[];
};

export type Entry = {
	revision: string;
	id: string;
	template: string;
	edition: number;
	document: Json;
};

export type Draft = {
	id: string;
	template: string;
	edition?: number;
	document: Json;
};

export type Operation =
	| { op: "set"; path: string; value: Json }
	| { op: "remove"; path: string };

export type Options = {
	access: () => string | undefined;
	fetcher?: typeof fetch;
};

const blocked = new Set(["preferences", "workbench", "tags"]);
const pattern = /^[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?$/;

export function reserved(id: string): boolean {
	return blocked.has(id);
}

export function valid(id: string): boolean {
	return pattern.test(id) && !blocked.has(id);
}

export function encode(token: string): string {
	return token.replaceAll("~", "~0").replaceAll("/", "~1");
}

function record(value: unknown): value is Record<string, Json> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function same(left: Json, right: Json): boolean {
	return JSON.stringify(left) === JSON.stringify(right);
}

function walk(
	prefix: string,
	before: Json,
	after: Json,
	list: Operation[],
): void {
	if (same(before, after)) return;
	if (!record(before) || !record(after)) {
		list.push({ op: "set", path: prefix, value: after });
		return;
	}
	for (const key of Object.keys(before)) {
		if (!Object.hasOwn(after, key)) {
			list.push({ op: "remove", path: `${prefix}/${encode(key)}` });
		}
	}
	for (const [key, value] of Object.entries(after)) {
		const path = `${prefix}/${encode(key)}`;
		if (!Object.hasOwn(before, key)) {
			list.push({ op: "set", path, value });
			continue;
		}
		walk(path, before[key], value, list);
	}
}

export function diff(before: Json, after: Json): Operation[] {
	const operations: Operation[] = [];
	walk("", before, after, operations);
	return operations;
}

export function parse(text: string): Json {
	try {
		return JSON.parse(text) as Json;
	} catch {
		throw new Failure(422, "文档内容必须是合法的 JSON。");
	}
}

export function list(options: Options): Promise<Listing> {
	return request<Listing>("/api/v1/me/documents", options);
}

export function read(id: string, options: Options): Promise<Entry> {
	return request<Entry>(
		`/api/v1/me/documents/${encodeURIComponent(id)}`,
		options,
	);
}

export function create(draft: Draft, options: Options): Promise<Entry> {
	const id = draft.id.trim();
	const template = draft.template.trim();
	if (!valid(id)) {
		return Promise.reject(
			new Failure(
				422,
				"文档标识必须为小写字母、数字或连字符，且不能使用保留名称 preferences、workbench 或 tags。",
			),
		);
	}
	if (!template || template === "meta") {
		return Promise.reject(
			new Failure(422, "请选择非 meta 的有效模板创建文档。"),
		);
	}
	return request<Entry>("/api/v1/me/documents", {
		...options,
		method: "POST",
		body: { ...draft, id, template },
	});
}

export function amend(
	input: { id: string; revision: string; operations: Operation[] },
	options: Options,
): Promise<Entry> {
	if (input.operations.length === 0) {
		return Promise.reject(new Failure(422, "没有需要提交的文档变更。"));
	}
	return request<Entry>(
		`/api/v1/me/documents/${encodeURIComponent(input.id)}`,
		{
			...options,
			method: "PATCH",
			body: { revision: input.revision, operations: input.operations },
		},
	);
}

export function migrate(
	input: { id: string; revision: string },
	options: Options,
): Promise<Entry> {
	return request<Entry>(
		`/api/v1/me/documents/${encodeURIComponent(input.id)}/migrate`,
		{
			...options,
			method: "POST",
			body: { revision: input.revision },
		},
	);
}

export function remove(
	input: { id: string; revision: string },
	options: Options,
): Promise<void> {
	const query = new URLSearchParams({ revision: input.revision }).toString();
	return request<void>(
		`/api/v1/me/documents/${encodeURIComponent(input.id)}?${query}`,
		{
			...options,
			method: "DELETE",
			empty: true,
		},
	);
}
