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
apps = []
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
one. A new root includes the `meta`, `theme` and `tags` templates at edition 1.
Keel holds the template and edition identities and their document references.
The `schemas` collection holds their Draft 2020-12 schema envelopes.
Bootstrap validates the meta-schema against itself and checks all registered
builtin bodies. Reopening verifies the same resource and document bindings before
serving.

Data roots created before template, `tags`, and personal structured document
bootstrap (including `folio:1`) use a preceding format (`folio:2` is current).
For cutover, set `data = "data-v2"` in `folio.toml` to select a new vacant
directory, run `folio-api bootstrap /path/to/config`, then run
`folio-api serve /path/to/config`. An existing root with a missing or preceding
format marker refuses to start without modifying `estate.sqlite3`. Keep
`format`, `estate.sqlite3`, and `documents/` together when retaining a root.
The settings routes and response shape continue to use the code Catalog.
A single process holds the data root. SIGINT/SIGTERM drain requests before
exit. `/health` performs an actual native resource read.
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

The initial executable declares `theme.family` (`google`, `google-accounts`,
`google-workspace`, `material`; default `google`) and `theme.tone` (`system`,
`light`, `dark`; default `system`), with the built-in `ensign` and `folio`
application scopes plus any optional validated slug scopes listed in `apps`. These
names describe preferences; Design owns the actual visual implementations.

A response from the theme catalog also carries `presentation`, the explicit
render a consumer hands to the Design Shell:

```json
{"system":"google-accounts","tone":"light","basis":"default"}
```

`system` is the closed Design system key for the effective family: the legacy
preference `google` renders as `google-accounts`, and the other family names map
to themselves. `tone` is only `light` or `dark`; the persistent `system`
preference is resolved on the server, never in the browser. `basis` names what
decided the tone: `setting` when an effective `light`/`dark` declaration (shared,
application or policy) decided it, `device` when `system` followed the allowed
device fact, and `default` when `system` had no device fact and light was chosen.

The optional `tone=light|dark` query parameter carries that one allowed device
fact and only applies while the effective preference is `system`. Any other
value returns 422 `invalid` without touching the document; the parameter is
never stored, so it cannot become a second preference or authority source. A
client sends it only when it has such a fact and otherwise omits it, which yields
the explicit `default` result. Explicit `light`/`dark` and trusted policy wins
over the device fact.

Documents written by 0.1 remain valid: the former `google` family stays an
allowed, resolvable choice and the stored values keep their meaning. Moving to
the explicit `google-accounts` or `google-workspace` family is an ordinary
declaration change; nothing is rewritten in place. Presentation is projected
by the theme catalog only, so a caller-supplied `Catalog` with unrelated
`theme.family` and `theme.tone` definitions keeps its previous response shape
and never fails on its own vocabulary.

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

## Confirmed preferences document

`GET /v1/me/documents/preferences?app=ensign&tone=dark` returns a single
`revision` alongside `document.shared` and `document.app`. Each view contains
its application name or null, evaluated `items` keyed by setting name, and the
presentation when available. Within the returned document,
`/app/items/theme.tone/effective` is a JSON Pointer with a literal dot in the
setting key. Each item retains explicit declaration presence. For a shared-only request, omit `app` to receive `document.app: null`.
Shared and app projections come from one authorized user-document snapshot.

`PATCH` on the same route takes the existing `revision` and `changes` body.
It returns the same envelope with the new revision and evaluated views of the
committed values. Both requested views are evaluated before the conditional edit.
Invalid changes, projection errors and stale heads leave values unchanged.
The client can use the returned document and revision directly as its confirmed
state. `tone` is a per-request device fact and the write still declares the scope
of each change.

Both methods require the caller's Ensign resource token and return
`Cache-Control: no-store`. Signed-out callers can obtain defaults from their host
application. The existing `/v1/me/settings` routes retain their response shape.

## Scopes, workbench, templates and structured documents

- `GET /v1/scopes` returns the registered scopes (`shared`, built-in `ensign` and
  `folio`, plus configured `apps` extensions) and catalog setting definitions.
- `GET` and `PATCH /v1/me/documents/workbench?tone=<light|dark>` (`Cache-Control:
  no-store`) return one atomic snapshot across `shared` and every registered
  application scope (`document.scopes`, `document.shared`, `document.apps.<app>`),
  accepting the same `{ revision, changes }` batch across any mix of scopes.
- `GET /v1/templates`, `GET /v1/templates/{name}`, and
  `GET /v1/templates/{name}/editions/{number}` inspect registered templates,
  immutable editions, and Draft 2020-12 schemas via the operator's native Keel
  Face.
- `GET /v1/templates/{name}/status` runs a non-mutating quick-fail compatibility
  check of the caller's visible documents against the template's `latest` Draft
  2020-12 schema.
- `POST /v1/templates` and `POST /v1/templates/{name}/editions` publish new
  templates and immutable editions strictly for Keel-granted managers (`meta` is
  immutable); schemas are validated against `meta` edition 1 and compiled as Draft
  2020-12.
- `GET` and `PATCH /v1/me/documents/tags` (`Cache-Control: no-store`) manage the
  caller's map-by-slug `tags` document using Radicula JSON Pointer `operations`
  validated against `tags` edition 1.
- `GET` and `POST /v1/me/documents`, `GET`, `PATCH`, and `DELETE
  /v1/me/documents/{id}`, and `POST /v1/me/documents/{id}/migrate` (`Cache-Control:
  no-store`) manage personal structured documents bound to any non-`meta` template
  edition with row-level owner isolation, Draft 2020-12 validation, optimistic CAS
  revisions, and explicit quick-fail migration to `latest`.

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
