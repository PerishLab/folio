import {
	type Address,
	type Client,
	connect,
	type State as Document,
	type Json,
	type Reply,
	type Request,
	type Snapshot,
	type Transport,
} from "@perishlab/radicula-document";
import { forget } from "./authorization.ts";
import { Failure, reply, uncertain } from "./client.ts";

export type { Client, Document, Json, Reply, Snapshot };
export type Tag = { label: string; color?: string };
export type Entry = { slug: string; label: string; color: string };
export type Operation =
	| { op: "set"; path: string; value: Tag }
	| { op: "remove"; path: string };
export type Options = {
	access: () => string | undefined;
	fetcher?: typeof fetch;
};

const base = "/api/v1/me/documents";
const conflict = "标签文档已在其他会话更新，已重新读取最新内容。";

export function encode(token: string): string {
	return token.replaceAll("~", "~0").replaceAll("/", "~1");
}

export function pointer(slug: string): string {
	return `/tags/${encode(slug)}`;
}

export function upsert(slug: string, label: string, color?: string): Operation {
	const value: Tag = { label: label.trim() };
	const tint = color?.trim();
	if (tint) value.color = tint;
	return { op: "set", path: pointer(slug.trim()), value };
}

export function erase(slug: string): Operation {
	return { op: "remove", path: pointer(slug.trim()) };
}

function address(target: Address): string {
	return `${target.base_rel_path}/${target.binding.key}`;
}

function transport(options: Options): Transport<Operation> {
	const fetcher = options.fetcher ?? fetch;
	return {
		async read(target: Address): Promise<Snapshot> {
			const token = options.access();
			if (!token) throw new Failure(401);
			const response = await fetcher(address(target), {
				method: "GET",
				cache: "no-store",
				credentials: "omit",
				redirect: "error",
				signal: AbortSignal.timeout(15000),
				headers: { authorization: `Bearer ${token}` },
			});
			if (response.status === 401) {
				forget();
				throw new Failure(401);
			}
			if (!response.ok) throw new Failure(response.status);
			return response.json();
		},
		async write(payload: Request<Operation>): Promise<Reply> {
			const token = options.access();
			if (!token) return { state: "rejected", error: new Failure(401) };
			const response = await fetcher(address(payload), {
				method: "PATCH",
				cache: "no-store",
				credentials: "omit",
				redirect: "error",
				signal: AbortSignal.timeout(15000),
				headers: {
					authorization: `Bearer ${token}`,
					"content-type": "application/json",
				},
				body: JSON.stringify({
					revision: payload.revision,
					operations: [payload.intent],
				}),
			});
			return reply(response, conflict);
		},
	};
}

export function open(options: Options): Client<Operation> {
	return connect<Operation>({
		base_rel_path: base,
		transport: transport(options),
		project: (body) => body,
	});
}

function record(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function entries(body: Json | undefined): Entry[] {
	if (!record(body) || !record(body.tags)) return [];
	const rows: Entry[] = [];
	for (const [slug, value] of Object.entries(body.tags)) {
		if (!record(value) || typeof value.label !== "string") continue;
		rows.push({
			slug,
			label: value.label,
			color: typeof value.color === "string" ? value.color : "",
		});
	}
	rows.sort((left, right) => left.slug.localeCompare(right.slug));
	return rows;
}

export function notice(state: Document): string | undefined {
	if (state.phase === "unknown") {
		return state.error instanceof Failure ? state.error.message : uncertain;
	}
	if (state.error instanceof Failure) return state.error.message;
	if (state.error === undefined) return undefined;
	return state.phase === "failed"
		? "暂时无法读取标签文档，请重试。"
		: "暂时无法完成标签更新，请重试。";
}
