# Rustex

Rustex is a Convex -> Rust code generation toolkit. The project goal is to
analyze as much of a Convex codebase as can be derived statically, normalize it
into a backend-neutral intermediate representation, and generate deterministic
Rust artifacts plus machine-readable manifests for future targets.

This README is the shared project plan and implementation tracker.

## Vision

Rustex should become a production-grade compiler/codegen toolchain for Convex
applications, with:

- database document generation from Convex schema validators
- request/response contract generation for queries, mutations, and actions
- internal function support where static analysis can recover contracts
- robust IR and manifest output for debugging, CI, and future backends
- deterministic Rust code generation with serde-friendly types
- diagnostics for unsupported or lossy constructs instead of silent guessing

## Scope

### v1

- Standard single-app Convex layout with one primary `convex/` root
- Static analysis of `schema.ts` and function modules under `convex/`
- Validator-driven extraction for args and returns
- Basic generated metadata consumption from `convex/_generated/*.d.ts`
- IR snapshot, manifest output, diagnostics output
- Rust generation for ids, document models, and API request/response contracts
- CLI commands for `generate`, `check`, `inspect`, and `diff`

### v2

- Better generated metadata reconciliation
- Optional TS-checker-based return inference with explicit provenance
- Components support
- JSON Schema / OpenAPI-like projections
- Incremental caching and watch-mode improvements
- More configurable naming, derives, and backend customization

### Later

- Additional target languages
- Plugin/override hooks
- Monorepo-first discovery
- IDE/LSP tooling
- Richer type recovery beyond validator-first extraction

### Out of scope

- Executing user Convex code during analysis
- Runtime evaluation of validator factories
- Full TypeScript semantic equivalence
- Reverse-engineering unstable/private Convex internals as a hard dependency

## Source Of Truth

Rustex should inspect these surfaces in priority order:

1. `convex/_generated/api.d.ts`, `dataModel.d.ts`, `server.d.ts`
2. `convex/schema.ts`
3. exported functions under `convex/**/*.ts`
4. imported/shared validator fragments
5. the TypeScript program graph and symbol table
6. version metadata from `package.json`, lockfiles, and installed Convex package metadata

Policy:

- Schema validators are truth for document models.
- Function `args` and `returns` validators are truth for request/response contracts.
- Convex-generated metadata is corroborating truth for naming and topology.
- TypeScript inference is fallback evidence only.

## Architecture

### Workspace

- `crates/rustex-cli`: CLI entrypoint and command orchestration
- `crates/rustex-project`: config loading and project layout resolution
- `crates/rustex-ts-analyzer`: Rust bridge to the Bun/TypeScript analyzer
- `crates/rustex-convex`: IR finalization and hashing
- `crates/rustex-ir`: language-agnostic IR model
- `crates/rustex-diagnostics`: structured diagnostics model
- `crates/rustex-rustgen`: Rust code generation backend
- `crates/rustex-output`: deterministic artifact writing
- `crates/rustex-testkit`: shared test utilities
- `packages/ts-analyzer`: Bun-executed TypeScript analyzer

### Extraction pipeline

1. Load `rustex.toml` and resolve the project layout.
2. Invoke the Bun/TypeScript analyzer from Rust.
3. Build a TypeScript `Program` for the Convex project.
4. Parse schema validators and exported Convex function registrations.
5. Normalize extracted shapes into IR.
6. Finalize and hash the IR deterministically.
7. Emit Rust code, IR JSON, manifest JSON, and diagnostics JSON.

### IR model

The current IR includes:

- project metadata
- tables
- functions
- source origins
- diagnostics
- manifest metadata
- type nodes for scalars, literals, ids, arrays, records, objects, unions, and unknown values

This layer exists so Rust generation is not coupled directly to TypeScript AST
details and future output targets can reuse the same normalized model.

### Rust codegen

The Rust backend currently emits:

- `Cargo.toml` for the generated crate
- `lib.rs`
- `ids.rs`
- `models.rs`
- `api.rs`

Current mapping policy:

- `v.object` -> Rust `struct` for document models and argument objects
- `v.id("table")` -> table-specific `TableId` newtype
- `v.array(T)` -> `Vec<T>`
- `v.record(v.string(), T)` -> `BTreeMap<String, T>`
- nullable unions of the form `T | null` -> `Option<T>`
- unsupported or ambiguous shapes -> `serde_json::Value`

## CLI

Primary commands:

```sh
cargo run -- generate
cargo run -- check
cargo run -- inspect functions --format json
cargo run -- diff
```

Analyzer-specific checks:

```sh
cd packages/ts-analyzer
bun install
bun run check
```

Default output location:

- `generated/rustex/`

Config file:

- `rustex.toml`

## Current Implementation Checklist

### Core scaffolding

- [x] Rust workspace and crate split
- [x] Top-level `rustex.toml`
- [x] Bun-based analyzer package
- [x] Example `convex/` app in the repo root

### CLI and orchestration

- [x] `generate`
- [x] `check`
- [x] `inspect`
- [x] `diff`
- [ ] `watch`
- [ ] `init`

### Project discovery

- [x] Resolve project root, Convex root, and output directory from config
- [x] Resolve analyzer script from the Rust workspace
- [ ] Rich layout detection for unsupported project shapes
- [ ] Monorepo/component-aware discovery

### TypeScript analyzer

- [x] Real TypeScript source file analyzer
- [x] Strict analyzer `tsconfig.json`
- [x] Bun runtime integration
- [x] Effect-based control flow
- [x] Effect-compatible CLI option parsing
- [x] TypeScript `Program` creation and symbol resolution
- [ ] Incremental compiler host / persistent cache
- [ ] Explicit generated metadata reconciliation logic
- [ ] TS-checker-based return inference

### Validator extraction

- [x] `v.string`
- [x] `v.number`
- [x] `v.int64`
- [x] `v.boolean`
- [x] `v.null`
- [x] `v.bytes`
- [x] `v.any`
- [x] `v.literal`
- [x] `v.id`
- [x] `v.array`
- [x] `v.record`
- [x] `v.object`
- [x] `v.union`
- [x] `v.optional`
- [x] imported identifier dereferencing for statically reducible expressions
- [x] object spread / shorthand flattening when statically resolvable
- [ ] richer diagnostics for opaque helper abstractions
- [ ] recursive/advanced validator edge-case handling

### Convex surface extraction

- [x] schema extraction from `defineSchema`
- [x] table extraction from `defineTable`
- [x] public `query` detection
- [x] public `mutation` detection
- [x] public `action` detection
- [x] `internalQuery` detection
- [x] `internalMutation` detection
- [x] `internalAction` detection
- [x] module/export based canonical function path construction
- [ ] HTTP action extraction
- [ ] component extraction
- [ ] generated API topology reconciliation

### IR and manifest

- [x] project metadata
- [x] table model
- [x] function model
- [x] origin/source spans
- [x] diagnostics list
- [x] manifest metadata with deterministic input hash
- [ ] explicit constraint model
- [ ] interned/shared named type graph
- [ ] capability flags and source inventory

### Rust backend

- [x] generated crate manifest
- [x] table id newtypes
- [x] document model generation
- [x] request argument struct generation
- [x] response alias generation
- [x] serde derives
- [x] basic field renaming to snake_case
- [x] `Option<T>` for optional/null unions when recognized
- [ ] dedicated enum generation for literal unions
- [ ] dedicated enum generation for discriminated unions
- [ ] nested object extraction into named Rust types
- [ ] stable collision handling beyond current simple casing strategy
- [ ] feature flags / custom derives / custom attributes

### Diagnostics

- [x] structured diagnostic type
- [x] warning for missing args validator
- [x] warning for missing returns validator
- [x] error for dynamic `v.id(...)`
- [ ] broader diagnostic taxonomy from the master plan
- [ ] human-friendly snippet rendering
- [ ] JSON/text parity guarantees across commands

### Outputs

- [x] generated Rust crate
- [x] `rustex.ir.json`
- [x] `rustex.manifest.json`
- [x] `rustex.diagnostics.json`
- [ ] source maps for generated symbol -> origin
- [ ] JSON Schema output
- [ ] OpenAPI-like output

### Testing

- [x] end-to-end smoke test driven by the root `convex/` example app
- [x] analyzer typecheck with Bun
- [x] `cargo test`
- [ ] golden tests for generated files
- [ ] broader fixture corpus
- [ ] compatibility matrix across Convex versions
- [ ] generated Rust compile-check as part of automated tests

## Current Limits

- Standard `convex/` layout only
- Validator-first extraction only
- No trustworthy handler-body return inference
- Generated metadata is detected and version-tagged, but not deeply reconciled
- Object and union codegen is intentionally conservative and still falls back to `serde_json::Value`
- No stable public customization surface yet

## Near-Term Priorities

1. Reconcile against `convex/_generated/*.d.ts` instead of only detecting them.
2. Replace lossy Rust fallbacks for literal unions and nested objects with named types.
3. Add golden tests and compile-check tests for generated Rust output.
4. Expand diagnostics to cover more unsupported and partial-generation cases.
5. Add watch mode and incremental analyzer caching.

## Repository Notes

- `convex/` is the single canonical example Convex app in the repository.
- Tests synthesize temporary fixture projects from that root example instead of
  committing duplicate Convex app copies.
- `generated/` is a runtime output directory and is not committed.
