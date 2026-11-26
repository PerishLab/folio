import assert from "node:assert/strict";
import test from "node:test";
import { Failure } from "../client.ts";
import { board, intent, notice, open, scopes } from "../workbench.ts";

function item(name, effective, source = "default", editable = true) {
	const options =
		name === "theme.tone"
			? ["system", "light", "dark"]
			: ["google-accounts", "google-workspace", "material"];
	return {
		name,
		options,
		shared: { state: "missing" },
		app: { state: "missing" },
		effective,
		source,
		editable,
	};
}

function snapshot(revision, system = "google-workspace", tone = "light") {
	return {
		revision,
		document: {
			scopes: ["ensign", "folio"],
			shared: {
				app: null,
				presentation: { system, tone, basis: "default" },
				items: {
					"theme.family": item("theme.family", system, "shared"),
					"theme.tone": item("theme.tone", tone, "default"),
				},
			},
			apps: {
				ensign: {
					app: "ensign",
					presentation: { system, tone, basis: "default" },
					items: {
						"theme.family": item("theme.family", system, "shared"),
						"theme.tone": item("theme.tone", tone, "default"),
					},
				},
				folio: {
					app: "folio",
					presentation: { system, tone, basis: "setting" },
					items: {
						"theme.family": item("theme.family", system, "app"),
						"theme.tone": item("theme.tone", tone, "app"),
					},
				},
			},
		},
	};
}

const json = (body, status = 200) => Response.json(body, { status });
const binding = (identity) => ({ key: "workbench", identity });
const settled = (state) => state.phase === "ready" && state.pending === 0;

function until(client, check) {
	if (check(client.state)) return Promise.resolve(client.state);
	return new Promise((resolve) => {
		const stop = client.subscribe(() => {
			if (!check(client.state)) return;
			stop();
			resolve(client.state);
		});
	});
}

test("scopes and workbench intent builders target shared and qualified app scopes", async () => {
	assert.deepEqual(intent("shared", "theme.tone"), {
		op: "reset",
		scope: { kind: "shared" },
		name: "theme.tone",
	});
	assert.deepEqual(intent("folio", "theme.family", "material"), {
		op: "set",
		scope: { kind: "app", name: "folio" },
		name: "theme.family",
		value: "material",
	});
	let seen: { url: unknown; init: RequestInit } | undefined;
	const found = await scopes({
		access: () => "tok",
		fetcher: async (url, init) => {
			seen = { url, init };
			return json({
				scopes: [
					{ kind: "shared", name: null, builtin: true },
					{ kind: "app", name: "folio", builtin: true },
				],
				definitions: [],
			});
		},
	});
	assert.equal(seen?.url, "/api/v1/scopes");
	assert.equal(seen?.init.headers.authorization, "Bearer tok");
	assert.equal(found.scopes.length, 2);
});

test("workbench reflects live Folio presentation and handles 409 conflict reload", async () => {
	let reads = 0;
	let writes = 0;
	const store = open({
		access: () => "held",
		device: () => "dark",
		fetcher: async (url, init) => {
			assert.equal(url, "/api/v1/me/documents/workbench?tone=dark");
			if (init.method === "GET") {
				reads += 1;
				return json(
					reads === 1
						? snapshot("r1", "google-workspace", "light")
						: snapshot("r2", "material", "dark"),
				);
			}
			writes += 1;
			if (writes === 1) return json({ error: "conflict" }, 409);
			return json(snapshot("r3", "google-accounts", "dark"));
		},
	});
	assert.equal(store.system.read().value, "google-workspace");
	assert.equal(store.tone.read().value, "dark");
	await store.client.select(binding("u1:dark:0"));
	assert.equal(board(store.client.state.document)?.scopes.length, 2);
	store.client.update(intent("folio", "theme.tone", "dark"));
	const conflicted = await until(
		store.client,
		(next) => settled(next) && next.confirmed?.revision === "r2",
	);
	assert.ok(conflicted.error instanceof Failure);
	assert.equal(conflicted.error.status, 409);
	assert.match(notice(conflicted) ?? "", /其他会话更新/);
	assert.equal(store.system.read().value, "material");
	assert.equal(store.tone.read().value, "dark");
	store.client.update(intent("folio", "theme.family", "google-accounts"));
	const saved = await until(
		store.client,
		(next) => settled(next) && next.confirmed?.revision === "r3",
	);
	assert.equal(saved.error, undefined);
	assert.equal(store.system.read().value, "google-accounts");
});

test("workbench freezes queue on uncertain 503 and recovers on new epoch", async () => {
	let reads = 0;
	const store = open({
		access: () => "held",
		device: () => "light",
		fetcher: async (_url, init) => {
			if (init.method === "GET") {
				reads += 1;
				return json(snapshot(`r${reads}`));
			}
			return json({ error: "unavailable" }, 503);
		},
	});
	await store.client.select(binding("u1:light:0"));
	store.client.update(intent("shared", "theme.tone", "dark"));
	const paused = await until(store.client, (next) => next.phase === "unknown");
	assert.equal(paused.pending, 1);
	assert.match(notice(paused) ?? "", /尚未确认/);
	assert.throws(() =>
		store.client.update(intent("shared", "theme.tone", "light")),
	);
	await store.client.select(binding("u1:light:1"));
	assert.equal(store.client.state.phase, "ready");
	assert.equal(store.client.state.confirmed?.revision, "r2");
});
