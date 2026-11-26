import { capture, Locus, type Options } from "@perishlab/locus";

export type Settings = Omit<Options, "producer" | "endpoint">;

export let locus = new Locus({ producer: "folio-web", enabled: false });
let release: (() => void) | undefined;
let active: string | undefined;

function stop(): Promise<void> {
	release?.();
	release = undefined;
	active = undefined;
	const prior = locus;
	locus = new Locus({ producer: "folio-web", enabled: false });
	return prior.dispose();
}

function clean(text: string): boolean {
	if (!text || /\s/u.test(text) || text.includes("\\")) return false;
	for (const char of text) {
		const point = char.codePointAt(0) ?? 0;
		if (point < 32 || (point >= 127 && point <= 159)) return false;
	}
	return true;
}

export function endpoint(raw: unknown): string | undefined {
	if (typeof raw !== "string") return undefined;
	const text = raw.trim();
	if (!text.startsWith("/") || text.startsWith("//")) return undefined;
	if (!clean(text)) return undefined;
	return text;
}

export function activate(raw?: string, options?: Settings): Locus {
	const target = endpoint(raw);
	if (!target) {
		if (active !== undefined) void stop();
		return locus;
	}
	if (active === target && !options) return locus;
	void stop();
	active = target;
	locus = new Locus({
		...options,
		producer: "folio-web",
		endpoint: target,
	});
	release = capture(locus);
	return locus;
}

export function reset(): Promise<void> {
	return stop();
}

export function safe(value: unknown): string | undefined {
	if (typeof value !== "string") return undefined;
	return /^[a-z0-9_.-]{1,64}$/i.test(value) ? value : undefined;
}

export function tally(series: string, labels?: Record<string, string>): void {
	locus.metric({
		name: "auth.callback",
		unit: "count",
		kind: "counter",
		series,
		value: 1,
		...(labels ? { labels } : {}),
	});
}

function family(agent: string): string {
	if (/edg/i.test(agent)) return "edge";
	if (/firefox|fxios/i.test(agent)) return "firefox";
	if (/chrome|crios|chromium/i.test(agent)) return "chrome";
	if (/safari/i.test(agent)) return "safari";
	return "other";
}

function browser(): string {
	try {
		return family(globalThis.navigator?.userAgent ?? "");
	} catch {
		return "other";
	}
}

function display(): string {
	try {
		return globalThis.matchMedia?.("(display-mode: standalone)").matches
			? "standalone"
			: "browser";
	} catch {
		return "browser";
	}
}

function framed(): string {
	try {
		return globalThis.top !== globalThis.self ? "true" : "false";
	} catch {
		return "true";
	}
}

function opener(): string {
	try {
		return globalThis.opener ? "true" : "false";
	} catch {
		return "false";
	}
}

function navigation(): string {
	try {
		const entry = globalThis.performance?.getEntriesByType?.("navigation")?.[0];
		if (!entry || !("type" in entry)) return "unknown";
		return typeof entry.type === "string" && entry.type
			? entry.type
			: "unknown";
	} catch {
		return "unknown";
	}
}

function referrer(): string {
	try {
		const value = globalThis.document?.referrer;
		return value ? new URL(value).origin : "";
	} catch {
		return "";
	}
}

function origin(): string {
	try {
		return globalThis.location?.origin ?? "";
	} catch {
		return "";
	}
}

export function environment(): Record<string, string> {
	return {
		browser: browser(),
		display: display(),
		framed: framed(),
		opener: opener(),
		navigation: navigation(),
		referrer: referrer(),
		origin: origin(),
	};
}

export function stored(verified: boolean, keys: number, backup: boolean): void {
	locus.with(
		{
			...environment(),
			verified: String(verified),
			keys: String(keys),
			backup: String(backup),
		},
		() => locus.info("auth start stored"),
	);
}

export function context(
	query: URLSearchParams | undefined,
	session: boolean,
	backup: boolean,
	keys: number,
): Record<string, string> {
	return {
		...environment(),
		hascode: String(Boolean(query?.get("code"))),
		hasstate: String(Boolean(query?.get("state"))),
		haserror: String(Boolean(query?.get("error"))),
		session: String(session),
		backup: String(backup),
		keys: String(keys),
	};
}
