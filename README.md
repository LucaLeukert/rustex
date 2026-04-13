# Rustex

Rustex is a Convex -> Rust code generation toolkit. This repository currently
contains the MVP compiler pipeline:

- Rust workspace with IR, diagnostics, project discovery, codegen, and CLI
- Node/TypeScript analyzer sidecar for statically reading Convex source
- Rust output generation for document models, ids, and request/response types
- JSON IR, manifest, and diagnostics output
- Smoke test that builds a temporary fixture from the repo's example `convex/` app

## Commands

```sh
cargo run -- generate
cargo run -- check
cargo run -- inspect functions --format json
```

## Current MVP limits

- Standard `convex/` layout only
- Validator-driven extraction only
- No handler return inference yet
- Generated metadata is detected and version-tagged, but only lightly reconciled
- Union/object codegen is intentionally conservative

## Repository layout

- `crates/` Rust workspace crates
- `packages/ts-analyzer/` Node analyzer worker
- `convex/` example Convex app used as the local integration target and test seed
