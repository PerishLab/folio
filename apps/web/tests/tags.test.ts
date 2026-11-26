import assert from "node:assert/strict";
import test from "node:test";
import { Failure } from "../client.ts";
import {
	encode,
	entries,
	erase,
	notice,
	open,
	pointer,
	upsert,
} from "../tags.ts";

const json = (body, status = 200) => Response.json(body, { status });
const binding = (identity) => ({ key: "tags", identity });
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

test("RFC 6901 pointer escaping handles ~ and / in tag slugs", () => {
	assert.equal(encode("work/a~b"), "work~1a~0b");
	assert.equal(pointer("team/ops~1"), "/tags/team~1ops~01");
	assert.deepEqual(upsert("team/ops", "运维", "#1a73e8"), {
		op: "set",
		path: "/tags/team~1ops",
		value: { label: "运维", color: "#1a73e8" },
	});
	assert.deepEqual(upsert("plain", "默认", "  "), {
		op: "set",
		path: "/tags/plain",
		value: { label: "默认" },
	});
	assert.deepEqual(erase("team/ops"), {
		op: "remove",
		path: "/tags/team~1ops",
	});
});

test("tags client applies set/remove operations and handles 409, 422 and 503", async () => {
	const requests = [];
	let mode = "conflict";
	let reads = 0;
	const failures = {
		conflict: () => {
			mode = "ok";
			return json({ error: "conflict" }, 409);
		},
		invalid: () => json({ error: "invalid" }, 422),
		down: () => json({ error: "unavailable" }, 503),
	};
	const client = open({
		access: () => "tok",
		fetcher: async (url, init) => {
			requests.push({ url, init });
			if (init.method === "GET") {
				reads += 1;
				return json({
					revision: `r${reads}`,
					template: "tags",
					edition: 1,
					document: {
						tags: {
							zeta: { label: "Zeta" },
							"alpha/1": { label: "Alpha", color: "blue" },
						},
					},
				});
			}
			const fail = failures[mode];
			if (fail) return fail();
			return json({
				revision: "r3",
				template: "tags",
				edition: 1,
				document: { tags: { "alpha/1": { label: "Alpha", color: "blue" } } },
			});
		},
	});
	await client.select(binding("u1:0"));
	const list = entries(client.state.document);
	assert.deepEqual(
		list.map((item) => item.slug),
		["alpha/1", "zeta"],
	);
	client.update(erase("zeta"));
	const conflicted = await until(
		client,
		(next) => settled(next) && next.confirmed?.revision === "r2",
	);
	assert.ok(conflicted.error instanceof Failure);
	assert.equal(conflicted.error.status, 409);
	client.update(erase("zeta"));
	const saved = await until(
		client,
		(next) => settled(next) && next.confirmed?.revision === "r3",
	);
	assert.equal(saved.error, undefined);
	assert.deepEqual(JSON.parse(requests[3].init.body), {
		revision: "r2",
		operations: [{ op: "remove", path: "/tags/zeta" }],
	});
	mode = "invalid";
	client.update(upsert("bad", ""));
	const rejected = await until(
		client,
		(next) => settled(next) && next.error !== undefined,
	);
	assert.ok(rejected.error instanceof Failure);
	assert.equal(rejected.error.status, 422);
	mode = "down";
	client.update(erase("alpha/1"));
	const paused = await until(client, (next) => next.phase === "unknown");
	assert.match(notice(paused) ?? "", /尚未确认/);
});
