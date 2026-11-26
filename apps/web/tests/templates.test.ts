import assert from "node:assert/strict";
import test from "node:test";
import { Failure } from "../client.ts";
import {
	create,
	detail,
	edition,
	list,
	parse,
	publish,
	status,
	valid,
} from "../templates.ts";

const json = (body, code = 200) => Response.json(body, { status: code });

test("template slug validation and JSON schema parser enforce strict bounds", () => {
	assert.equal(valid("notes-v2"), true);
	assert.equal(valid("-bad"), false);
	assert.equal(valid("UPPER"), false);
	assert.deepEqual(parse('{"type":"object"}'), { type: "object" });
	assert.throws(() => parse("[]"), Failure);
	assert.throws(() => parse("{bad"), Failure);
});

test("templates client reads catalog, editions, status and surfaces 403 on unauthorized publish", async () => {
	const seen = [];
	const routes = {
		"GET /api/v1/templates": {
			templates: [{ name: "theme", latest: 1, editions: [1], builtin: true }],
		},
		"GET /api/v1/templates/theme": {
			name: "theme",
			builtin: true,
			latest: 1,
			editions: [{ number: 1, document: 2, revision: "rev1" }],
		},
		"GET /api/v1/templates/theme/editions/1": {
			template: "theme",
			number: 1,
			document: 2,
			revision: "rev1",
			schema: { type: "object" },
		},
		"GET /api/v1/templates/theme/status": {
			template: "theme",
			latest: 1,
			format: "folio:2",
			ready: true,
			checked: 1,
			compatible: 1,
			incompatible: [],
		},
	};
	const options = {
		access: () => "tok",
		fetcher: async (url, init) => {
			seen.push({ url: String(url), method: init.method });
			const hit = routes[`${init.method} ${url}`];
			return hit ? json(hit) : json({ error: "denied" }, 403);
		},
	};
	const catalog = await list(options);
	assert.equal(catalog.templates[0].name, "theme");
	const info = await detail({ name: "theme" }, options);
	assert.equal(info.latest, 1);
	const spec = await edition({ name: "theme", number: 1 }, options);
	assert.equal(spec.schema.type, "object");
	const report = await status({ name: "theme" }, options);
	assert.equal(report.format, "folio:2");
	assert.equal(report.ready, true);
	await assert.rejects(
		create({ name: "meta", schema: {} }, options),
		(error) => error instanceof Failure && error.status === 422,
	);
	await assert.rejects(
		create({ name: "custom", schema: { type: "object" } }, options),
		(error) => error instanceof Failure && error.status === 403,
	);
	await assert.rejects(
		publish({ name: "meta", base: 1, schema: {} }, options),
		(error) => error instanceof Failure && error.status === 403,
	);
	assert.equal(seen.length, 5);
});
