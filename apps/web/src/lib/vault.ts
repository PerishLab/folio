export type Vault = Pick<Storage, "getItem" | "setItem" | "removeItem"> & {
	readonly length?: number;
};

export const slot = "folio.authorization";
export const pending = "folio.authorization.pending";

type Entry = {
	state: string;
	until: number;
	data: string;
};

const ttl = 300_000;
const cap = 4;

export function local(): Vault | undefined {
	try {
		return globalThis.localStorage;
	} catch {
		return undefined;
	}
}

function session(): Vault | undefined {
	try {
		return globalThis.sessionStorage;
	} catch {
		return undefined;
	}
}

function valid(item: unknown, now: number): item is Entry {
	if (typeof item !== "object" || item === null) return false;
	const entry = item as Record<string, unknown>;
	if (typeof entry.state !== "string" || entry.state.length === 0) return false;
	if (typeof entry.until !== "number" || !Number.isFinite(entry.until)) {
		return false;
	}
	if (entry.until <= now) return false;
	return typeof entry.data === "string" && entry.data.length > 0;
}

function load(store: Vault, now: number): Entry[] {
	try {
		const raw = store.getItem(pending);
		if (!raw) return [];
		const parsed = JSON.parse(raw);
		if (!Array.isArray(parsed)) return [];
		return parsed.filter((item): item is Entry => valid(item, now));
	} catch {
		return [];
	}
}

function write(store: Vault, entries: Entry[]): boolean {
	try {
		if (entries.length === 0) {
			store.removeItem(pending);
		} else {
			store.setItem(pending, JSON.stringify(entries));
		}
		return true;
	} catch {
		return false;
	}
}

function drop(store: Vault | undefined, key: string): void {
	if (!store) return;
	try {
		store.removeItem(key);
	} catch {
		return;
	}
}

export class Locker {
	private readonly storage: Vault;
	private readonly backup?: Vault;

	constructor(storage: Vault, fallback?: Vault) {
		this.storage = storage;
		const primary = session();
		this.backup =
			fallback !== undefined
				? fallback
				: primary && storage === primary
					? local()
					: undefined;
	}

	count(): number {
		try {
			return typeof this.storage.length === "number" ? this.storage.length : 0;
		} catch {
			return 0;
		}
	}

	save(
		state: string,
		encoded: string,
	): { verified: boolean; backed: boolean; keys: number } {
		this.storage.setItem(slot, encoded);
		let backed = false;
		if (this.backup) {
			const now = Date.now();
			const kept = load(this.backup, now).filter(
				(entry) => entry.state !== state,
			);
			kept.push({ state, until: now + ttl, data: encoded });
			backed = write(this.backup, kept.slice(-cap));
		}
		const verified = Boolean(this.storage.getItem(slot));
		return { verified, backed, keys: this.count() };
	}

	pull(): string | null {
		const raw = this.storage.getItem(slot);
		this.storage.removeItem(slot);
		return raw;
	}

	take(state?: string): string | undefined {
		if (!this.backup) return undefined;
		try {
			if (!this.backup.getItem(pending)) return undefined;
			const fresh = load(this.backup, Date.now());
			const found = state
				? fresh.find((entry) => entry.state === state)
				: undefined;
			const kept = found ? fresh.filter((entry) => entry !== found) : fresh;
			write(this.backup, kept);
			return found?.data;
		} catch {
			return undefined;
		}
	}
}

export function clear(): void {
	drop(session(), slot);
	drop(local(), pending);
}
