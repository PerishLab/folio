import {
	type Config,
	configuration,
	forget,
	held,
	Refusal,
	start,
	subscribe as watch,
} from "../../authorization.ts";
import {
	type Board,
	board,
	type Change,
	type Document,
	intent,
	scopes as load,
	notice,
	open,
	type Presentation,
	type Scopes,
	type Store,
	type Tone,
} from "../../workbench.ts";

export type State = {
	ready: boolean;
	config?: Config;
	signed: boolean;
	warning: string;
	system: string;
	tone: Tone;
	basis: string;
	document: Document;
	board?: Board;
	scopes?: Scopes;
	toasts: string[];
	mood: "calm" | "warn";
};

const listeners = new Set<() => void>();
let booted = false;
let ready = false;
let config: Config | undefined;
let warning = "";
let epoch = 0;
let catalog: Scopes | undefined;
let toasts: string[] = [];
let mood: "calm" | "warn" = "calm";
let timer: ReturnType<typeof setTimeout> | undefined;
let store: Store | undefined;

function emit(): void {
	for (const listener of listeners) listener();
}

export function device(): Tone {
	return globalThis.matchMedia?.("(prefers-color-scheme: dark)").matches
		? "dark"
		: "light";
}

function ensure(): Store {
	if (store) return store;
	store = open({ access: held, device });
	store.client.subscribe(() => emit());
	return store;
}

function read(
	handle: { read: () => { state: string; value?: string } },
	fallback: string,
): string {
	const result = handle.read();
	return result.state === "value" && typeof result.value === "string"
		? result.value
		: fallback;
}

function live(): Presentation {
	const current = ensure();
	const raw = read(current.tone, device());
	const tone: Tone = raw === "dark" ? "dark" : "light";
	const origin = read(current.basis, "default");
	const basis: Presentation["basis"] =
		origin === "setting" || origin === "device" ? origin : "default";
	return {
		system: read(current.system, "google-workspace"),
		tone,
		basis,
	};
}

export function snapshot(): State {
	const current = ensure();
	const shown = live();
	const doc = current.client.state;
	return {
		ready,
		config,
		signed: held() !== undefined,
		warning,
		system: shown.system,
		tone: shown.tone,
		basis: shown.basis,
		document: doc,
		board: board(doc.document),
		scopes: catalog,
		toasts,
		mood,
	};
}

export function subscribe(listener: () => void): () => void {
	listeners.add(listener);
	return () => listeners.delete(listener);
}

export function announce(text: string, kind: "calm" | "warn" = "calm"): void {
	if (timer) clearTimeout(timer);
	mood = kind;
	toasts = [text];
	emit();
	timer = setTimeout(() => {
		toasts = [];
		emit();
	}, 3500);
}

export async function sync(force = false): Promise<void> {
	const current = ensure();
	const token = held();
	if (!token) {
		catalog = undefined;
		await current.client.select(null);
		emit();
		return;
	}
	if (force) epoch += 1;
	const identity = `${token.slice(0, 16)}:${device()}:${epoch}`;
	await current.client.select({ key: "workbench", identity });
	try {
		catalog = await load({ access: held });
	} catch {
		catalog = undefined;
	}
	emit();
}

export function boot(): void {
	if (booted) {
		void sync(false);
		return;
	}
	booted = true;
	ensure();
	watch(() => {
		void sync(false);
	});
	const media = globalThis.matchMedia?.("(prefers-color-scheme: dark)");
	media?.addEventListener?.("change", () => {
		void sync(false);
	});
	void configuration()
		.then((found) => {
			config = found;
			warning = "";
			ready = true;
			emit();
			return sync(false);
		})
		.catch((error) => {
			warning =
				error instanceof Refusal
					? error.message
					: "暂时无法读取客户端配置，请刷新重试。";
			ready = true;
			emit();
		});
}

export async function enter(back: string): Promise<void> {
	if (!config) return;
	try {
		const target = await start(config, {
			origin: location.origin,
			back,
			storage: sessionStorage,
			crypto,
		});
		location.assign(target);
	} catch (error) {
		announce(
			error instanceof Refusal ? error.message : "暂时无法发起登录，请重试。",
			"warn",
		);
	}
}

export function leave(): void {
	forget();
	catalog = undefined;
	void ensure().client.select(null);
	announce("已退出当前会话。");
}

export function navigate(href: string): void {
	if (location.pathname === href) return;
	history.pushState(null, "", href);
	dispatchEvent(new PopStateEvent("popstate"));
}

export function apply(scope: string, name: string, value?: string): void {
	const current = ensure();
	const phase = current.client.state.phase;
	if (phase === "unknown") {
		announce(
			"保存结果尚未确认，已暂停继续写入；请先点击刷新以确认当前状态。",
			"warn",
		);
		return;
	}
	if (phase !== "ready") {
		announce("设置快照尚未就绪，请先重新读取后再修改。", "warn");
		return;
	}
	const change: Change = intent(scope, name, value);
	current.client.update(change);
}

export function message(state: State): string | undefined {
	return notice(state.document);
}
