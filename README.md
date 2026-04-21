# Rustex

Rustex is a monorepo for bringing Convex-grade end-to-end type safety to Rust.
It has two core responsibilities:

- analyze a Convex TypeScript app and generate Rust API/model bindings
- provide a runtime crate that uses those generated bindings to make the Rust
  Convex client type-safe

The goal is not just “Rust codegen”. The goal is a Rust developer experience
closer to the Convex TypeScript SDK, where function names, argument shapes, and
response types flow through the client API instead of living as unchecked
strings and untyped values.

This README is the shared project plan and implementation tracker.

## Concrete Runtime Goal

Today, raw Convex Rust calls look like this:

```rust
let result = client.query("tasks:get", BTreeMap::new()).await?;
```

That is not safe because:

- `"tasks:get"` is just a string
- the argument shape is unchecked
- the return type is not encoded in the call site

Rustex should generate and support calls like this instead:

```rust
use rustex_generated::api::messages;
use rustex_runtime::RustexClient;

let mut client = RustexClient::new(&deployment_url).await?;
let result = rustex_generated::mutation!(client, messages::add, {
        author: "alice",
        body: "hello",
    })
    .await?;
```

In that model:

- the function path comes from generated code
- the args type is tied to that function
- the output type is tied to that function
- unsupported or unknown contracts still degrade explicitly to `serde_json::Value`
  instead of pretending to be strongly typed

## Vision

Rustex should become a production-grade compiler/codegen toolchain for Convex
applications, with:

- database document generation from Convex schema validators
- generated typed function specs for queries, mutations, and actions
- a runtime wrapper around the official `convex` crate
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
- Rust generation for ids, document models, API request/response contracts, and
  typed function descriptors
- Runtime crate with a typed wrapper over `convex::ConvexClient`
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
- `crates/rustex-ts-analyzer`: Rust bridge to the Node/TypeScript analyzer
- `crates/rustex-convex`: IR finalization and hashing
- `crates/rustex-ir`: language-agnostic IR model
- `crates/rustex-diagnostics`: structured diagnostics model
- `crates/rustex-runtime`: typed runtime wrapper over the official Rust Convex client
- `crates/rustex-rustgen`: Rust code generation backend
- `crates/rustex-swiftgen`: Swift code generation backend with bundled Swift runtime sources
- `crates/rustex-output`: deterministic artifact writing
- `crates/rustex-testkit`: shared test utilities
- `packages/ts-analyzer`: Effect-based TypeScript analyzer bundled to a single JavaScript file for Node

### Extraction pipeline

1. Load `rustex.toml` and resolve the project layout.
2. Bundle the Node/TypeScript analyzer and invoke it from Rust.
3. Build a TypeScript `Program` for the Convex project.
4. Parse schema validators and exported Convex function registrations.
5. Normalize extracted shapes into IR.
6. Finalize and hash the IR deterministically.
7. Emit target code, including typed function specs consumed by the Rust or Swift runtime.
8. Emit IR JSON, manifest JSON, and diagnostics JSON.

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
- `api.rs` with typed function specs

Current mapping policy:

- `v.object` -> Rust `struct` for document models and argument objects
- `v.id("table")` -> table-specific `TableId` newtype
- `v.array(T)` -> `Vec<T>`
- `v.record(v.string(), T)` -> `BTreeMap<String, T>`
- nullable unions of the form `T | null` -> `Option<T>`
- unsupported or ambiguous shapes -> `serde_json::Value`

Current generated API policy:

- each Convex module becomes a Rust submodule inside `api`
- each function gets a zero-sized generated marker type
- each function marker implements a runtime trait such as `QuerySpec`,
  `MutationSpec`, or `ActionSpec`
- args/output types are attached to the function marker through trait
  associated types

### Runtime

`rustex-runtime` wraps the official `convex` crate and provides:

- `RustexClient`
- `FunctionSpec`, `QuerySpec`, `MutationSpec`, `ActionSpec`
- argument serialization into `convex::Value`
- typed response decoding from `convex::FunctionResult`
- explicit runtime errors for transport issues, function errors, invalid arg
  encoding, and deserialization failures

### Swift codegen

The Swift backend emits a Swift Package under `<out_dir>/swift` when
`emit = ["swift"]` is configured. The package contains:

- a `RustexRuntime` target that wraps the official `ConvexMobile.ConvexClient`
- a generated app bindings target, defaulting to `RustexGenerated`
- typed ids, document models, function args, and function responses
- typed query subscription, one-shot query, mutation, and action helpers

The Swift runtime mirrors the Rust runtime shape where Convex Swift exposes the
same capability: `RustexFunctionSpec`, `RustexQuerySpec`,
`RustexMutationSpec`, `RustexActionSpec`, `RustexClient`, typed operation
methods, structured runtime errors, and raw Convex client access. Convex Swift
does not expose Rust's `watch_all`; the generated runtime exposes
`watchWebSocketState()` and keeps `raw` public for direct Convex Swift features.

Swift generation is enabled from `rustex.toml`:

```toml
emit = ["swift", "manifest", "ir"]

[swift]
package_name = "RustexGenerated"
module_name = "RustexGenerated"
product_name = "RustexGenerated"
runtime_module_name = "RustexRuntime"
client_facade_name = "RustexClient"
generate_package = true
bundle_runtime = true
access_level = "public"
tools_version = "5.10"
unknown_type_strategy = "any_codable"
emit_doc_comments = true
convex_dependency_url = "https://github.com/get-convex/convex-swift"

[swift.convex_dependency_requirement]
kind = "from"
version = "0.8.1"
```

Generated Swift uses Convex Swift's documented numeric wrappers such as
`@ConvexInt`, `@OptionalConvexInt`, `@ConvexFloat`, and
`@OptionalConvexFloat` for decoded values.

## CLI

### Install

For consumers, the primary install path is the Rust CLI package:

```sh
cargo install rustex-cli
rustex --help
```

The Cargo package is named `rustex-cli` because `rustex` is already taken on
crates.io by an unrelated crate. The installed executable is still named
`rustex`.

Generated Rust code depends on the published runtime crate:

```toml
[dependencies]
rustex-runtime = "0.1.0"
```

The generated crate's `Cargo.toml` includes this runtime dependency by default
when the CLI is installed from crates.io. Local workspace builds use the adjacent
`crates/rustex-runtime` path automatically so repository tests and examples can
compile without publishing first.

Publishing order for a crates.io release:

1. `rustex-diagnostics`
2. `rustex-ir`
3. `rustex-project`
4. `rustex-convex`
5. `rustex-rustgen`
6. `rustex-swiftgen`
7. `rustex-output`
8. `rustex-ts-analyzer`
9. `rustex-runtime`
10. `rustex-cli`

`rustex-ts-analyzer` ships a vendored analyzer bundle for published Cargo
installs. Development builds inside this monorepo still rebuild the analyzer
from `packages/ts-analyzer` with Node, pnpm, and esbuild.

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
pnpm install
npm run check
```

Default output location:

- `generated/rustex/`

Config file:

- `rustex.toml`

## Current Implementation Checklist

### Core scaffolding

- [x] Rust workspace and crate split
- [x] Top-level `rustex.toml`
- [x] TypeScript analyzer package
- [x] Example `convex/` app in the repo root
- [x] Monorepo runtime crate

### CLI and orchestration

- [x] `generate`
- [x] `check`
- [x] `inspect`
- [x] `diff`
- [x] `watch`
- [x] `init`

### Project discovery

- [x] Resolve project root, Convex root, and output directory from config
- [x] Resolve analyzer script from the Rust workspace
- [x] Rich layout detection for unsupported project shapes
- [x] Monorepo/component-aware discovery

### TypeScript analyzer

- [x] Real TypeScript source file analyzer
- [x] Strict analyzer `tsconfig.json`
- [x] Node runtime integration for the analyzer bridge
- [x] Effect-based control flow
- [x] Effect-compatible CLI option parsing
- [x] TypeScript `Program` creation and symbol resolution
- [x] Incremental compiler host / persistent cache
- [x] Explicit generated metadata reconciliation logic
- [x] TS-checker-based return inference

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
- [x] richer diagnostics for opaque helper abstractions
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
- [x] HTTP action extraction
- [x] component extraction
- [x] generated API topology reconciliation

### IR and manifest

- [x] project metadata
- [x] table model
- [x] function model
- [x] origin/source spans
- [x] diagnostics list
- [x] manifest metadata with deterministic input hash
- [x] explicit constraint model
- [x] interned/shared named type graph
- [x] capability flags and source inventory

### Rust backend

- [x] generated crate manifest
- [x] table id newtypes
- [x] document model generation
- [x] request argument struct generation
- [x] response alias generation
- [x] generated module-scoped function descriptors
- [x] generated runtime trait impls for function descriptors
- [x] serde derives
- [x] basic field renaming to snake_case
- [x] `Option<T>` for optional/null unions when recognized
- [x] dedicated enum generation for literal unions
- [x] dedicated enum generation for discriminated unions
- [x] nested object extraction into named Rust types
- [x] stable collision handling beyond current simple casing strategy
- [x] feature flags / custom derives / custom attributes

### Diagnostics

- [x] structured diagnostic type
- [x] warning for missing args validator
- [x] warning for missing returns validator
- [x] error for dynamic `v.id(...)`
- [ ] broader diagnostic taxonomy from the master plan
- [x] human-friendly snippet rendering
- [ ] JSON/text parity guarantees across commands

### Outputs

- [x] generated Rust crate
- [x] `rustex.ir.json`
- [x] `rustex.manifest.json`
- [x] `rustex.diagnostics.json`
- [x] generated runtime-facing typed API surface
- [x] source maps for generated symbol -> origin
- [x] JSON Schema output
- [x] OpenAPI-like output

### Testing

- [x] end-to-end smoke test driven by the root `convex/` example app
- [x] analyzer typecheck with TypeScript/Node
- [x] `cargo test`
- [x] runtime conversion tests
- [x] smoke test coverage for generated typed API descriptors
- [x] golden tests for generated files
- [x] broader fixture corpus
- [ ] compatibility matrix across Convex versions
- [x] generated Rust compile-check as part of automated tests

## Current Limits

- Standard `convex/` layout plus common monorepo auto-discovery
- Validator-first extraction only
- Handler-body return inference is available, but still fallback-oriented and less trustworthy than explicit validators
- Generated metadata is reconciled at the API topology level, not yet deeply semantically merged
- Object and union codegen is stronger for nested objects and common unions, but still falls back to `serde_json::Value` for unsupported shapes
- No stable public customization surface yet
- The runtime can only be as type-safe as the extracted/generated contracts

## Near-Term Priorities

1. Reconcile against `convex/_generated/*.d.ts` instead of only detecting them.
2. Replace lossy Rust fallbacks for literal unions, nested objects, and returns
   with named types.
3. Expand the runtime surface to cover subscriptions and query sets safely.
4. Add golden tests and compile-check tests for generated Rust output.
5. Expand diagnostics to cover more unsupported and partial-generation cases.
6. Add watch mode and incremental analyzer caching.

## Repository Notes

- `convex/` is the single canonical example Convex app in the repository.
- Tests synthesize temporary fixture projects from that root example instead of
  committing duplicate Convex app copies.
- `generated/` is a runtime output directory and is not committed.
