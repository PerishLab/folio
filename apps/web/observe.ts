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

export function activate(endpoint?: string, options?: Settings): Locus {
	const target = typeof endpoint === "string" ? endpoint.trim() : "";
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
