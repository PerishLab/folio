# Folio

Folio declares settings in trusted Rust code and serves evaluated results. Ensign
proves external identity, Keel controls resources and persists user documents,
Radicula defines values and exact edits, and Design renders controlled properties.
The browser consumes Folio's result and submits intent; it does not recompute
inheritance, policy, permissions or effective values.

## Run

Create a caller-owned directory containing `folio.toml` and an independently
qualified snapshot of your Ensign issuer's public JWKS in `jwks.json`:

```toml
issuer = "https://ensign.example/api"
audience = "folio"
keys = "jwks.json"
data = "data"
listen = "127.0.0.1:8080"
```

Register `folio` as an Ensign resource audience. The official `ensign-client`
SDK retains login custody and exchanges a resource access token for `folio`.
Folio accepts that token as `Authorization: Bearer <resource-access-token>`.
Its serving process receives public keys only. Issuer, audience, access-token
kind, ES256 signature and expiry must match. Key rotation currently requires
updating the trusted public snapshot and restarting Folio.

```sh
folio-api bootstrap /path/to/config
folio-api serve /path/to/config
```

Bootstrap initializes a vacant data directory or verifies a complete existing
one. Serve requires that initialized directory. Preserve `estate.sqlite3` and
`documents/` together. A single process holds the data root; SIGINT/SIGTERM drain
requests before exit. `/health` performs an actual native resource read.
Configuration uses Radicula Cascade and Plumb resolution, under the `API_`
prefix. Relative keys and data paths resolve against the supplied config root.

## API

`GET /v1/me/settings?app=ensign` returns one user-document revision and evaluated
items. Omitting `app` selects the shared context. A single item is available at
`GET /v1/me/settings/theme.tone?app=ensign`, with the same revision and item shape.
The caller cannot select a different subject.

Each item contains `name`, visible `options`, `shared` and `app` declarations,
`effective`, `source`, and `editable`. A declaration is `{"state":"missing"}`
or `{"state":"present","value":...}`; explicit null is distinct from missing.
`source` is `default`, `shared`, `app`, or `policy`. `editable` describes the
trusted code policy in the selected context. Every write independently passes
through the mapped operator's native Keel authority and can return 403.

The initial executable declares `theme.family` (`google`, `material`; default
`google`) and `theme.tone` (`system`, `light`, `dark`; default `system`), with the
`ensign` application scope. These names describe preferences; Design owns the
actual visual implementations.

`PATCH /v1/me/settings?app=ensign` takes an opaque expected revision and changes:

```json
{
  "revision": "<revision from GET>",
  "changes": [
    {"op":"set","scope":{"kind":"shared"},"name":"theme.tone","value":"dark"},
    {"op":"set","scope":{"kind":"app","name":"ensign"},"name":"theme.tone","value":"light"}
  ]
}
```

Use `op: "reset"` without `value` to remove a declaration and resume inheritance.
`op: "patch"` accepts `operations` of `set` (path and value) and `remove` (path),
using Radicula's exact JSON pointer paths inside an already declared value.
The declared setting validates the resulting value. All changes in one user's
batch publish once or leave the document unchanged; stale revisions return 409.
The query selects the response context; each change explicitly selects its scope.

Evaluation applies defaults, shared values, application overrides, then trusted
code policy. A policy may restrict choices, force a value or deny edits and may
inspect the complete declaration snapshot. Historical declarations remain visible;
values no longer valid for the declaration or policy do not become effective.
If policy choices exclude every inherited candidate, the first allowed choice
becomes the policy result. Web clients consume these results directly.

Errors use stable `error` categories: 401 unauthorized, 403 denied, 404 missing,
409 conflict, 422 invalid and 503 unavailable/corrupt. Malformed HTTP input uses
the router's extraction errors. No driver text enters response bodies.

## Embed and deploy

The public `folio` crate accepts explicit paths, `Catalog` declarations and a
trusted policy callback; it discovers no environment or config. Its identity
mapping qualifies issuer plus external subject, never treats a numeric subject
as a native row id, and persists that mapping across restarts. There is no
cross-user transaction or distributed storage promise.

Wharf distributes the crate, Linux `folio-api`, its OCI image and `charts/folio`
through Plumb's declaration. The Helm chart runs one replica with Recreate,
initializes the data root in an init container and mounts a public JWKS ConfigMap
and one persistent volume. Supply `issuer` and `jwks`, and select an existing
claim or storage class. Add routing in the consuming environment's declaration.
Do not mount Ensign signing keys, sudo custody or held login bearers.
