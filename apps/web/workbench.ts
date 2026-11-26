import {
	type Address,
	type Client,
	connect,
	type State as Document,
	type Handle,
	type Json,
	type Reply,
	type Request,
	type Snapshot,
	type Transport,
} from "@perishlab/radicula-document";
import { forget } from "./authorization.ts";
import { Failure, reply, request, uncertain } from "./client.ts";

export type { Client, Document, Handle, Json, Reply, Snapshot };
export type Tone = "light" | "dark";
export type Declaration =
	| { state: "missing" }
	| { state: "present"; value: string };
export type Item = {
	name: string;
	options: string[];
	shared: Declaration;
	app: Declaration;
	effective: string;
	source: "default" | "shared" | "app" | "policy";
	editable: boolean;
};
export type Presentation = {
	system: string;
	tone: Tone;
	basis: "setting" | "device" | "default";
};
export type View = {
	app: string | null;
	items: Record<string, Item>;
	presentation?: Presentation;
};
export type Board = {
	scopes: string[];
	shared: View;
	apps: Record<string, View>;
};
export type Slot = {
	kind: string;
	name: string | null;
	builtin: boolean;
};
export type Summary = {
	name: string;
	default: Json;
	options: Json[];
};
export type Scopes = {
	scopes: Slot[];
	definitions: Summary[];
};
export type Change = {
	op: "set" | "reset";
	scope: { kind: "shared" } | { kind: "app"; name: string };
	name: string;
	value?: string;
};
export type Options = {
	access: () => string | undefined;
	device: () => Tone;
	fetcher?: typeof fetch;
};
export type Store = {
	client: Client<Change>;
	system: Handle<string>;
	tone: Handle<string>;
	basis: Handle<string>;
};

const base = "/api/v1/me/documents";
const conflict = "设置已在其他会话更新，已重新读取最新状态。";

function address(target: Address, tone: Tone): string {
	return `${target.base_rel_path}/${target.binding.key}?tone=${tone}`;
}

function transport(options: Options): Transport<Change> {
	const fetcher = options.fetcher ?? fetch;
	return {
		async read(target: Address): Promise<Snapshot> {
			const token = options.access();
			if (!token) throw new Failure(401);
			const response = await fetcher(address(target, options.device()), {
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
			if (!response.ok) {
				throw new Failure(
					response.status,
					response.status >= 500 ? "暂时无法读取全域设置，请重试。" : undefined,
				);
			}
			return response.json();
		},
		async write(payload: Request<Change>): Promise<Reply> {
			const token = options.access();
			if (!token) return { state: "rejected", error: new Failure(401) };
			const response = await fetcher(address(payload, options.device()), {
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
					changes: [payload.intent],
				}),
			});
			return reply(response, conflict);
		},
	};
}

export function open(options: Options): Store {
	const client = connect<Change>({
		base_rel_path: base,
		transport: transport(options),
		project: (body) => body,
	});
	return {
		client,
		system: client.string(
			"/apps/folio/presentation/system",
			() => "google-workspace",
		),
		tone: client.string("/apps/folio/presentation/tone", options.device),
		basis: client.string("/apps/folio/presentation/basis", () => "default"),
	};
}

export function scopes(
	options: Pick<Options, "access" | "fetcher">,
): Promise<Scopes> {
	return request<Scopes>("/api/v1/scopes", options);
}

export function intent(scope: string, name: string, value?: string): Change {
	return {
		op: value === undefined ? "reset" : "set",
		scope:
			scope === "shared" ? { kind: "shared" } : { kind: "app", name: scope },
		name,
		...(value === undefined ? {} : { value }),
	};
}

function record(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function board(body: Json | undefined): Board | undefined {
	if (!record(body)) return undefined;
	if (!Array.isArray(body.scopes)) return undefined;
	if (!record(body.shared) || !record(body.apps)) return undefined;
	if (!record(body.shared.items)) return undefined;
	return body as unknown as Board;
}

export function notice(state: Document): string | undefined {
	if (state.phase === "unknown") {
		return state.error instanceof Failure ? state.error.message : uncertain;
	}
	if (state.error instanceof Failure) return state.error.message;
	if (state.error === undefined) return undefined;
	return state.phase === "failed"
		? "暂时无法读取全域设置，请重试。"
		: "暂时无法完成操作，请重新读取后重试。";
}
