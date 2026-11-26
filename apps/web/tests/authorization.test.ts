import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import test from "node:test";
import {
	complete,
	configuration,
	forget,
	held,
	start,
	subscribe,
} from "../authorization.ts";
import { Failure, request } from "../client.ts";

const config = {
	issuer: "https://ensign.example/api",
	client: "folio-web",
	audience: "folio",
	authorize: "https://ensign.example/api/authorize",
	token: "https://ensign.example/api/token",
};
const origin = "https://folio.example";

function vault() {
	sessionStorage.clear();
	return sessionStorage;
}

async function journey(back = "/") {
	const storage = vault();
	const url = new URL(
		await start(config, { origin, back, storage, crypto: webcrypto }),
	);
	return {
		storage,
		url,
		callback: `${origin}/callback?code=once&state=${url.searchParams.get("state")}`,
	};
}

test("unconfigured client.json returns undefined and discovery binds issuer", async () => {
	assert.equal(await configuration(async () => Response.json({})), undefined);
	await assert.rejects(
		configuration(async () => Response.json([])),
		/身份配置/,
	);
	const urls = [];
	const discovered = await configuration(async (url) => {
		urls.push(String(url));
		if (urls.length === 1) {
			return Response.json({
				issuer: config.issuer,
				client: config.client,
				endpoint: "https://proxy.example/oidc",
			});
		}
		return Response.json({
			issuer: config.issuer,
			authorization_endpoint: config.authorize,
			token_endpoint: config.token,
		});
	});
	assert.deepEqual(discovered, {
		...config,
		token: "https://proxy.example/oidc/token",
	});
	assert.equal(
		urls[1],
		"https://proxy.example/oidc/.well-known/openid-configuration",
	);
	let step = 0;
	await assert.rejects(
		configuration(async () =>
			Response.json(
				++step === 1
					? { issuer: config.issuer, client: config.client }
					: {
							issuer: "https://foreign.example/api",
							authorization_endpoint: config.authorize,
							token_endpoint: config.token,
						},
			),
		),
		/签发方/,
	);
});

test("PKCE S256 binds verifier, redirect, resource audience and allowlisted return paths", async () => {
	for (const route of ["/", "/tags", "/templates", "/documents"]) {
		const { storage, url } = await journey(route);
		const saved = JSON.parse(storage.getItem("folio.authorization"));
		const digest = await webcrypto.subtle.digest(
			"SHA-256",
			new TextEncoder().encode(saved.verifier),
		);
		assert.equal(
			url.searchParams.get("code_challenge"),
			Buffer.from(digest).toString("base64url"),
		);
		assert.equal(url.searchParams.get("code_challenge_method"), "S256");
		assert.equal(url.searchParams.get("redirect_uri"), `${origin}/callback`);
		assert.equal(url.searchParams.get("resource"), "folio");
		assert.equal(url.searchParams.get("client_id"), config.client);
		assert.equal(saved.back, route);
		assert.equal("access_token" in saved, false);
	}
	await assert.rejects(journey("//foreign.example"), /返回地址/);
	await assert.rejects(journey("/admin"), /返回地址/);
	await assert.rejects(journey("/tags/"), /返回地址/);
	await assert.rejects(journey("/tags?x=1"), /返回地址/);
	await assert.rejects(journey("https://evil.com"), /返回地址/);
	await assert.rejects(
		start(
			{ ...config, authorize: "javascript:alert(1)" },
			{ origin, back: "/", storage: vault(), crypto: webcrypto },
		),
		/授权地址/,
	);
});

test("tampered callback consumes one-shot PKCE state without token exchange", async () => {
	for (const alter of [
		(url) => url.replace(/state=.*/, "state=wrong"),
		(url) => `${url}&state=duplicate`,
		(url) => url.replace(/&state=.*/, ""),
		(url) => `${url}&error=access_denied`,
		(url) => url.replace(origin, "https://foreign.example"),
	]) {
		const { storage, callback } = await journey("/templates");
		let calls = 0;
		const seat = {
			url: alter(callback),
			storage,
			fetcher: async () => {
				calls += 1;
				throw Error("unexpected exchange");
			},
		};
		await assert.rejects(complete(config, seat), /不匹配/);
		assert.equal(calls, 0);
		assert.equal(storage.length, 0);
		await assert.rejects(complete(config, seat), /失效/);
	}
});

test("token exchange keeps bearer strictly in memory, notifies subscribers and prevents replay", async () => {
	const { storage, callback } = await journey("/documents");
	let notices = 0;
	const stop = subscribe(() => {
		notices += 1;
	});
	let sent: { url: unknown; init: RequestInit } | undefined;
	const fetcher = async (url, init) => {
		sent = { url, init };
		return Response.json({
			access_token: "bearer-token",
			token_type: "Bearer",
			expires_in: 120,
		});
	};
	assert.equal(
		await complete(config, { url: callback, storage, fetcher }),
		"/documents",
	);
	assert.equal(sent.url, config.token);
	assert.equal(sent.init.credentials, "omit");
	assert.equal(sent.init.redirect, "error");
	assert.equal(sent.init.body.get("grant_type"), "authorization_code");
	assert.equal(held(), "bearer-token");
	assert.equal(storage.length, 0);
	assert.ok(notices >= 1);
	await assert.rejects(
		request("/api/v1/scopes", {
			access: held,
			fetcher: async () =>
				Response.json({ error: "unauthorized" }, { status: 401 }),
		}),
		(error) => error instanceof Failure && error.status === 401,
	);
	assert.equal(held(), undefined);
	await assert.rejects(
		complete(config, { url: callback, storage, fetcher }),
		/失效/,
	);
	assert.equal(held(), undefined);
	forget();
	stop();
});
