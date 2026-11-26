import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import test from "node:test";
import { gunzipSync } from "node:zlib";
import { complete, forget, start, type Vault } from "../authorization.ts";
import { activate, locus, reset } from "../observe.ts";

const pending = "folio.authorization.pending";

type Atom = {
	producer: string;
	context: Record<string, string>;
	payload?: Record<string, unknown>;
};

const config = {
	issuer: "https://ensign.example/api",
	client: "folio-web",
	audience: "folio",
	authorize: "https://ensign.example/api/authorize",
	token: "https://ensign.example/api/token",
};
const origin = "https://folio.example";

function memory(): Vault {
	const map = new Map<string, string>();
	return Object.defineProperty(
		Object.fromEntries([
			["getItem", (key: string) => map.get(key) ?? null],
			["setItem", (key: string, value: string) => map.set(key, value)],
			["removeItem", (key: string) => map.delete(key)],
		]),
		"length",
		{ get: () => map.size },
	) as Vault;
}

function decode(body: BodyInit | null | undefined, gzipped: boolean): string {
	const raw =
		body instanceof Uint8Array
			? Buffer.from(body)
			: Buffer.from(String(body ?? ""));
	return gzipped ? gunzipSync(raw).toString("utf8") : raw.toString("utf8");
}

function collector() {
	const atoms: Atom[] = [];
	const lines: string[] = [];
	const fetcher: typeof fetch = async (_input, init) => {
		const headers = (init?.headers ?? {}) as Record<string, string>;
		const gzipped = headers["Content-Encoding"] === "gzip";
		const text = decode(init?.body, gzipped);
		for (const line of text.split("\n").filter(Boolean)) {
			lines.push(line);
			atoms.push(JSON.parse(line) as Atom);
		}
		return new Response(null, { status: 204 });
	};
	return { atoms, lines, fetcher };
}

function bearer(): typeof fetch {
	return async () =>
		Response.json({
			access_token: "fallback-secret-bearer-888",
			token_type: "Bearer",
			expires_in: 120,
		});
}

test("local fallback recovers missing session storage with diagnostics and prevents replay", async () => {
	const sink = collector();
	activate("/api/observe", {
		fetcher: sink.fetcher,
		database: null,
		mode: "stack",
	});
	const storage = memory();
	const fallback = memory();
	try {
		const auth = new URL(
			await start(config, {
				origin,
				back: "/tags",
				storage,
				fallback,
				crypto: webcrypto,
			}),
		);
		const saved = JSON.parse(storage.getItem("folio.authorization") ?? "{}");
		const state = auth.searchParams.get("state") ?? "";
		const verifier = String(saved.verifier ?? "");
		const code = "fallback-auth-code-555";
		const callback = `${origin}/callback?code=${code}&state=${state}`;
		storage.removeItem("folio.authorization");
		assert.equal(
			await complete(config, {
				url: callback,
				storage,
				fallback,
				fetcher: bearer(),
			}),
			"/tags",
		);
		assert.equal(fallback.getItem(pending), null);
		await assert.rejects(
			complete(config, {
				url: callback,
				storage,
				fallback,
				fetcher: bearer(),
			}),
			/本地会话缺失/,
		);
		await locus.flush();
		const started = sink.atoms.find(
			(atom) =>
				atom.context["locus.schema"] === "locus.log" &&
				atom.payload?.message === "auth start stored",
		);
		assert.ok(started);
		assert.equal(started.context.verified, "true");
		assert.equal(started.context.backup, "true");
		assert.equal(started.context.keys, "1");
		const recovered = sink.atoms.find(
			(atom) =>
				atom.context["metric.name"] === "auth.callback" &&
				atom.context["metric.series"] === "ok.fallback",
		);
		assert.ok(recovered);
		assert.ok(recovered.context.browser);
		assert.ok(recovered.context.navigation);
		assert.equal(recovered.context.session, "false");
		assert.equal(recovered.context.backup, "true");
		assert.equal(recovered.context.hascode, "true");
		assert.equal(recovered.context.hasstate, "true");
		assert.equal(recovered.context.haserror, "false");
		const replayed = sink.atoms.find(
			(atom) =>
				atom.context["metric.name"] === "auth.callback" &&
				atom.context["metric.series"] === "storage.missing",
		);
		assert.ok(replayed);
		assert.equal(replayed.context.session, "false");
		assert.equal(replayed.context.backup, "false");
		const serialized = sink.lines.join("\n");
		for (const secret of [
			code,
			state,
			verifier,
			"fallback-secret-bearer-888",
		]) {
			assert.ok(secret.length > 0);
			assert.equal(serialized.includes(secret), false);
		}
	} finally {
		forget();
		await reset();
	}
});

test("expired local fallback is rejected and pruned", async () => {
	const sink = collector();
	activate("/api/observe", {
		fetcher: sink.fetcher,
		database: null,
		mode: "stack",
	});
	const storage = memory();
	const fallback = memory();
	try {
		const auth = new URL(
			await start(config, {
				origin,
				back: "/documents",
				storage,
				fallback,
				crypto: webcrypto,
			}),
		);
		const state = auth.searchParams.get("state") ?? "";
		const encoded = storage.getItem("folio.authorization") ?? "";
		storage.removeItem("folio.authorization");
		fallback.setItem(
			pending,
			JSON.stringify([{ state, until: Date.now() - 1000, data: encoded }]),
		);
		await assert.rejects(
			complete(config, {
				url: `${origin}/callback?code=once&state=${state}`,
				storage,
				fallback,
				fetcher: bearer(),
			}),
			/本地会话缺失/,
		);
		assert.equal(fallback.getItem(pending), null);
		await locus.flush();
		const missing = sink.atoms.find(
			(atom) =>
				atom.context["metric.name"] === "auth.callback" &&
				atom.context["metric.series"] === "storage.missing",
		);
		assert.ok(missing);
		assert.equal(missing.context.session, "false");
		assert.equal(missing.context.backup, "false");
	} finally {
		forget();
		await reset();
	}
});

test("mismatched state in local fallback is not used and cap keeps 4 recent entries", async () => {
	const sink = collector();
	activate("/api/observe", {
		fetcher: sink.fetcher,
		database: null,
		mode: "stack",
	});
	const storage = memory();
	const fallback = memory();
	try {
		const states: string[] = [];
		for (let index = 0; index < 5; index += 1) {
			const auth = new URL(
				await start(config, {
					origin,
					back: "/",
					storage,
					fallback,
					crypto: webcrypto,
				}),
			);
			states.push(auth.searchParams.get("state") ?? "");
		}
		const stored = JSON.parse(fallback.getItem(pending) ?? "[]") as {
			state: string;
		}[];
		assert.equal(stored.length, 4);
		assert.deepEqual(
			stored.map((entry) => entry.state),
			states.slice(1),
		);
		storage.removeItem("folio.authorization");
		await assert.rejects(
			complete(config, {
				url: `${origin}/callback?code=once&state=wrong-state-value`,
				storage,
				fallback,
				fetcher: bearer(),
			}),
			/本地会话缺失/,
		);
		const remaining = JSON.parse(fallback.getItem(pending) ?? "[]") as {
			state: string;
		}[];
		assert.equal(remaining.length, 4);
		await locus.flush();
		const missing = sink.atoms.find(
			(atom) =>
				atom.context["metric.name"] === "auth.callback" &&
				atom.context["metric.series"] === "storage.missing",
		);
		assert.ok(missing);
		assert.equal(missing.context.backup, "false");
	} finally {
		forget();
		await reset();
	}
});
