import assert from "node:assert/strict";
import test from "node:test";
import { Failure } from "../client.ts";
import {
	amend,
	create,
	diff,
	list,
	migrate,
	read,
	remove,
	reserved,
	valid,
} from "../documents.ts";

const json = (body, status = 200) => Response.json(body, { status });

test("reserved slugs and meta template are blocked and diff emits RFC 6901 patches", async () => {
	for (const name of ["preferences", "workbench", "tags"]) {
		assert.equal(reserved(name), true);
		assert.equal(valid(name), false);
	}
	assert.equal(valid("note-1"), true);
	const patch = diff(
		{ keep: 1, drop: "old", nested: { "a/b": "v1", "c~d": true } },
		{ keep: 1, added: "new", nested: { "a/b": "v2", "c~d": true } },
	);
	assert.deepEqual(patch, [
		{ op: "remove", path: "/drop" },
		{ op: "set", path: "/added", value: "new" },
		{ op: "set", path: "/nested/a~1b", value: "v2" },
	]);
	assert.deepEqual(diff({ list: [1, 2] }, { list: [1, 3] }), [
		{ op: "set", path: "/list", value: [1, 3] },
	]);
	assert.deepEqual(diff("old", "new"), [{ op: "set", path: "", value: "new" }]);
	assert.deepEqual(diff({}, { toString: "literal", constructor: "safe" }), [
		{ op: "set", path: "/toString", value: "literal" },
		{ op: "set", path: "/constructor", value: "safe" },
	]);
	assert.deepEqual(diff({ toString: "literal" }, {}), [
		{ op: "remove", path: "/toString" },
	]);
	const options = { access: () => "tok", fetcher: async () => json({}) };
	await assert.rejects(
		create({ id: "tags", template: "theme", document: {} }, options),
		(error) => error instanceof Failure && error.status === 422,
	);
	await assert.rejects(
		create({ id: "doc-1", template: "meta", document: {} }, options),
		(error) => error instanceof Failure && error.status === 422,
	);
});

test("documents CRUD, migrate and CAS delete invoke exact Folio API routes", async () => {
	const calls = [];
	const options = {
		access: () => "tok",
		fetcher: async (url, init) => {
			calls.push({ url: String(url), method: init.method, body: init.body });
			if (init.method === "DELETE") return new Response(null, { status: 204 });
			if (url === "/api/v1/me/documents" && init.method === "GET") {
				return json({
					documents: [
						{ id: "doc-1", template: "theme", edition: 1, revision: "r1" },
					],
				});
			}
			if (url === "/api/v1/me/documents/doc-1/migrate") {
				return json({
					revision: "r3",
					id: "doc-1",
					template: "theme",
					edition: 2,
					document: {},
				});
			}
			return json({
				revision: "r2",
				id: "doc-1",
				template: "theme",
				edition: 1,
				document: { "theme.tone": "dark" },
			});
		},
	};
	const listing = await list(options);
	assert.equal(listing.documents.length, 1);
	const entry = await read("doc-1", options);
	assert.equal(entry.id, "doc-1");
	const created = await create(
		{ id: "doc-1", template: "theme", document: { "theme.tone": "dark" } },
		options,
	);
	assert.equal(created.revision, "r2");
	const patched = await amend(
		{
			id: "doc-1",
			revision: "r2",
			operations: [{ op: "set", path: "/theme.tone", value: "light" }],
		},
		options,
	);
	assert.equal(patched.revision, "r2");
	const upgraded = await migrate({ id: "doc-1", revision: "r2" }, options);
	assert.equal(upgraded.edition, 2);
	await remove({ id: "doc-1", revision: "r3" }, options);
	assert.equal(calls.at(-1).url, "/api/v1/me/documents/doc-1?revision=r3");
	assert.equal(calls.at(-1).method, "DELETE");
	await assert.rejects(
		remove(
			{ id: "doc-1", revision: "r3" },
			{
				access: () => "tok",
				fetcher: async () => json({ error: "down" }, 503),
			},
		),
		(error) =>
			error instanceof Failure && error.status === 503 && error.uncertain,
	);
	await assert.rejects(
		amend(
			{
				id: "doc-1",
				revision: "r3",
				operations: [{ op: "set", path: "/a", value: 1 }],
			},
			{
				access: () => "tok",
				fetcher: async () => {
					throw new TypeError("network lost");
				},
			},
		),
		(error) =>
			error instanceof Failure && error.status === 0 && error.uncertain,
	);
});
