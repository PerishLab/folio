import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import test from "node:test";
import { gunzipSync } from "node:zlib";
import { complete, configuration, forget, start } from "../authorization.ts";
import { Failure, request } from "../client.ts";
import { activate, endpoint, locus, reset } from "../observe.ts";

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
	const endpoints: string[] = [];
	const fetcher: typeof fetch = async (input, init) => {
		endpoints.push(String(input));
		const headers = (init?.headers ?? {}) as Record<string, string>;
		const gzipped = headers["Content-Encoding"] === "gzip";
		const text = decode(init?.body, gzipped);
		for (const line of text.split("\n").filter(Boolean)) {
			lines.push(line);
			atoms.push(JSON.parse(line) as Atom);
		}
		return new Response(null, { status: 204 });
	};
	return { atoms, lines, endpoints, fetcher };
}

function vault() {
	sessionStorage.clear();
	return sessionStorage;
}

async function begin(back = "/") {
	const storage = vault();
	const url = new URL(
		await start(config, { origin, back, storage, crypto: webcrypto }),
	);
	const saved = JSON.parse(storage.getItem("folio.authorization") ?? "{}");
	const state = url.searchParams.get("state") ?? "";
	return {
		storage,
		state,
		verifier: String(saved.verifier ?? ""),
		callback: `${origin}/callback?code=secret-auth-code-999&state=${state}`,
	};
}

test("client.json activates observe when present and stays no-op when absent", async () => {
	await reset();
	let calls = 0;
	const original = globalThis.fetch;
	globalThis.fetch = async () => {
		calls += 1;
		return new Response(null, { status: 204 });
	};
	try {
		assert.equal(await configuration(async () => Response.json({})), undefined);
		locus.info("silent");
		await locus.flush();
		assert.equal(calls, 0);
		assert.equal(
			await configuration(async () =>
				Response.json({ observe: "/api/observe" }),
			),
			undefined,
		);
		locus.info("active");
		await locus.flush();
		assert.ok(calls >= 1);
	} finally {
		await reset();
		globalThis.fetch = original;
	}
});

test("callback emits structured diagnostics without leaking secrets", async () => {
	const sink = collector();
	activate("/api/observe", { fetcher: sink.fetcher, database: null });
	const secrets: string[] = ["secret-auth-code-999", "super-secret-bearer-777"];
	try {
		await assert.rejects(
			complete(config, {
				url: `${origin}/callback?code=secret-auth-code-999&state=missing`,
				storage: vault(),
				fetcher: async () => new Response(null, { status: 500 }),
			}),
			/本地会话缺失/,
		);
		const mismatch = await begin("/tags");
		secrets.push(mismatch.state, mismatch.verifier);
		await assert.rejects(
			complete(config, {
				url: `${origin}/callback?code=secret-auth-code-999&state=wrong-state-xyz`,
				storage: mismatch.storage,
				fetcher: async () => new Response(null, { status: 500 }),
			}),
			/不匹配/,
		);
		const failed = await begin("/templates");
		secrets.push(failed.state, failed.verifier);
		await assert.rejects(
			complete(config, {
				url: failed.callback,
				storage: failed.storage,
				fetcher: async () =>
					Response.json({ error: "invalid_grant" }, { status: 400 }),
			}),
			/凭证交换失败/,
		);
		const ok = await begin("/documents");
		secrets.push(ok.state, ok.verifier);
		assert.equal(
			await complete(config, {
				url: ok.callback,
				storage: ok.storage,
				fetcher: async () =>
					Response.json({
						access_token: "super-secret-bearer-777",
						token_type: "Bearer",
						expires_in: 120,
					}),
			}),
			"/documents",
		);
		await locus.flush();
		const started = sink.atoms.find(
			(atom) =>
				atom.context["locus.schema"] === "locus.log" &&
				atom.payload?.message === "auth start stored",
		);
		assert.ok(started);
		assert.equal(started.context.verified, "true");
		assert.equal(started.context.keys, "1");
		assert.ok(started.context.browser);
		assert.ok(started.context.navigation);
		const series = sink.atoms
			.filter((atom) => atom.context["metric.name"] === "auth.callback")
			.map((atom) => atom.context["metric.series"]);
		assert.ok(series.includes("storage.missing"));
		assert.ok(series.includes("state.mismatch"));
		assert.ok(series.includes("token.failed"));
		assert.ok(series.includes("ok"));
		const fault = sink.atoms.find(
			(atom) =>
				atom.context["locus.schema"] === "locus.fault" &&
				atom.payload?.message === "token exchange failed",
		);
		assert.ok(fault);
		assert.equal(
			(fault.payload?.detail as Record<string, unknown>)?.status,
			400,
		);
		assert.equal(
			(fault.payload?.detail as Record<string, unknown>)?.reason,
			"invalid_grant",
		);
		const serialized = sink.lines.join("\n");
		for (const leaked of secrets) {
			assert.ok(leaked.length > 0);
			assert.equal(serialized.includes(leaked), false);
		}
	} finally {
		forget();
		await reset();
	}
});

test("client request records faults on 500 and network errors without leaking bearer", async () => {
	const sink = collector();
	activate("/api/observe", { fetcher: sink.fetcher, database: null });
	const bearer = "api-bearer-secret-42";
	try {
		await assert.rejects(
			request("/api/v1/scopes?token=dropme", {
				access: () => bearer,
				fetcher: async () => new Response(null, { status: 503 }),
			}),
			(error) => error instanceof Failure && error.status === 503,
		);
		await assert.rejects(
			request("/api/v1/scopes", {
				access: () => bearer,
				fetcher: async () => {
					throw new Error("socket closed");
				},
			}),
			(error) => error instanceof Failure && error.status === 0,
		);
		await locus.flush();
		const faults = sink.atoms.filter(
			(atom) => atom.context["locus.schema"] === "locus.fault",
		);
		assert.ok(faults.length >= 2);
		const serialized = sink.lines.join("\n");
		assert.equal(serialized.includes(bearer), false);
		assert.equal(serialized.includes("dropme"), false);
	} finally {
		await reset();
	}
});

test("observe rejects non-relative, protocol-relative, whitespace, backslash, and control endpoints", async () => {
	await reset();
	const invalid = [
		"https://evil.example/observe",
		"http://evil.example/observe",
		"//evil.example/observe",
		"api/observe",
		"/api/observe bad",
		"/api/\tobserve",
		"/api/\nobserve",
		"/api/observe\u0000",
		"/api/observe\u001f",
		"/api/observe\u007f",
		"/api/observe\u0085",
		"/api\\observe",
		"",
		"   ",
	];
	let calls = 0;
	const original = globalThis.fetch;
	globalThis.fetch = async () => {
		calls += 1;
		return new Response(null, { status: 204 });
	};
	try {
		assert.equal(endpoint(" /api/observe "), "/api/observe");
		for (const candidate of invalid) {
			assert.equal(endpoint(candidate), undefined, candidate);
			activate(candidate);
			locus.info("ignored");
			await locus.flush();
		}
		assert.equal(calls, 0);
		activate("/api/observe");
		await configuration(async () =>
			Response.json({ observe: "https://evil.example/observe" }),
		);
		locus.info("disabled");
		await locus.flush();
		assert.equal(calls, 0);
	} finally {
		await reset();
		globalThis.fetch = original;
	}
});
