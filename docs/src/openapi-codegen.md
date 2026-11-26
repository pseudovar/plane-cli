# OpenAPI codegen (optional)

Generated Rust types can track your **self-hosted** Plane schema without hand-maintaining every DTO. `plane-core` should stay the place for HTTP, caching, pagination, and envelopes; generated code is usually a **types-only** crate consumed by `plane-core` or used to cross-check responses.

## 1. Export the schema

From a running instance:

- Swagger: `http://<host>/api/schema/swagger-ui/`
- OpenAPI JSON/YAML: often under `http://<host>/api/schema/` (check your Plane version).

This repository does not include the schema. Pass your instance's schema URL, or a file you exported, to the script below. It is copied into `gen/`, which is gitignored.

## 2. Generate with OpenAPI Generator (Docker)

Example: generate a **Rust** client (or models-only) into `gen/plane-openapi`:

```bash
./scripts/gen-openapi-types.sh https://plane.example.com/api/schema/
# or a local file
./scripts/gen-openapi-types.sh ./plane-schema.yaml
# or PLANE_OPENAPI_SPEC=<path-or-url> ./scripts/gen-openapi-types.sh
```

The script uses the official `openapitools/openapi-generator-cli` image. Adjust the generator name and options inside the script for:

- `rust` (reqwest client)
- `rust-axum` / other targets
- **models only** if you split generators

## 3. Progenitor (Rust-native, reqwest)

For a strongly-typed **reqwest** client from OpenAPI 3.x:

1. Add a workspace member crate, e.g. `plane-openapi-gen`.
2. Use [progenitor](https://github.com/oxidecomputer/progenitor) with your spec in `build.rs`.

This fits well if you later replace ad-hoc `serde_json::Value` in `plane-core` with generated types **incrementally** (start with `Issue`, `Project`, paginated wrappers).

## 4. Practical rollout

1. Keep **`PlaneApi`** as the public facade.
2. Introduce generated types behind `TryFrom<Value>` or internal deserialization in one method at a time.
3. Run codegen in CI only when the spec file changes, or on a manual workflow, to avoid blocking every PR on Docker.

See [`scripts/gen-openapi-types.sh`](https://github.com/pseudovar/plane-cli/blob/main/scripts/gen-openapi-types.sh) for a minimal Docker invocation you can extend.
