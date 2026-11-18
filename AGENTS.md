# Folio

Read the canonical [PerishLab delivery governance](https://github.com/PerishLab/.github/blob/main/GOVERNANCE.md)
at work start and again before delivery or Issue closure. It owns organization-wide
Issue, pull and acceptance policy.

Folio owns code-declared settings, contextual evaluation and explicit API results.
Radicula owns document values, exact paths, presence and atomic edits. Keel owns
resource identity, authority, lifecycle and document execution. Ensign proves
identity. Design renders controlled component properties and user intents.

## Product boundary

- `crates/lib` publishes `folio`: declarations, trusted code policy, evaluation,
  user documents and qualified external identity mapping. It accepts explicit
  paths and inputs, with no environment or configuration discovery.
- `crates/api` owns HTTP projection and the `folio-api` executable. Only that
  executable resolves configuration with Radicula Cascade and Plumb prefix/value.
- Each user has one document in native keel-document. Its shared and application
  maps contain declared values, with absent entries meaning inheritance. A reset
  removes a declaration. Explicit null remains a value when the declaration allows it.
- Evaluation precedence is default, shared user value, application override,
  then trusted code policy. API results carry visible options, both declaration
  states, effective value, source, editability and one document revision.
  Editability describes trusted policy for the selected application context;
  every mutation still passes through the operator's native Keel Face.
- A mutation batch validates every change against declared settings and application
  scopes, computes one candidate document and publishes one expected-head edit.
  All changes in that user document commit together. Aggregate reads use that same
  document snapshot; no multi-user transaction is promised.
- Numeric-looking external subjects remain external strings. The qualified issuer
  and subject map to a local frozen Actor row. Anonymous native registration is
  reachable only after the API verifies an Ensign ES256 access token for the exact
  configured issuer and resource audience. JWT signing keys and held login bearers
  never enter Folio. Configured JWKS contains only trusted public verification keys.
- Genesis seeds registration and owner-only document creation grants. Runtime
  identity lookup uses the anonymous native face; document effects use the mapped
  operator face. The API exposes settings operations, not generic Keel resources.
- Bootstrap initializes a vacant data root or verifies an already complete root.
  Serve opens an existing root. Partial or conflicting roots retain their evidence
  and refuse. Keep the SQLite estate and redb payload directory together.
- One process owns one root. The server drains requests on SIGINT/SIGTERM before
  exit. HTTP health performs a native resource read. Public errors expose stable
  categories; driver details do not enter response bodies.
- Source code carries no comments. Keep names, functions and modules within the
  Ectropy policy. Add vocabulary only for necessary foreign interfaces.

## Delivery

Use the Issue's Concord Member and exact Claims. Normal Git hooks carry Guard;
Concord proves the Boundary and delivers the source. Run the full applicable
Guard before landing. Cargo manifests retain version 0.0.0; Wharf binds releases.
The first product distributes `folio`, the Linux `folio-api` binary and its OCI
image and Helm chart. Runtime configuration and public JWKS are caller-owned mounts.
