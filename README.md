# Rustex

Rustex generates typed Rust and Swift clients for a Convex app.

It reads a Convex TypeScript project, extracts schema validators and exported
query/mutation/action contracts, normalizes them into an intermediate
representation, and writes generated client packages for the configured targets.
The generated clients use small runtime layers over the official Convex clients,
so application code can call Convex functions with typed paths, typed arguments,
and typed results instead of raw strings and unstructured values.

## What It Generates

Rust output is a generated Cargo crate under `<out_dir>/rust`:

- table id newtypes
- document models from `schema.ts`
- argument and response types for Convex functions
- typed function specs for queries, mutations, and actions
- JS-like helper macros for typed calls

Swift output is a generated Swift package under `<out_dir>/swift`:

- a bundled `RustexRuntime` target over `ConvexMobile`
- a generated bindings target, defaulting to `RustexGenerated`
- table ids, models, arguments, responses, and API specs
- typed query, subscription, mutation, and action helpers

Rustex can also emit supporting artifacts:

- `rustex.ir.json`
- `rustex.manifest.json`
- `rustex.diagnostics.json`
- JSON Schema and OpenAPI-like projections

## Quick Start

Install or run the CLI:

```sh
cargo install rustex-cli
rustex --help
```

From a source checkout, use the workspace binary:

```sh
cargo run -p rustex-cli -- --help
```

Create a `rustex.toml` in your Convex project:

```toml
project_root = "."
convex_root = "./convex"
out_dir = "./generated/rustex"
emit = ["rust", "swift", "manifest", "ir", "diagnostics"]
strict = false
allow_inferred_returns = true
naming_strategy = "safe"
id_style = "newtype_per_table"

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

Generate outputs:

```sh
rustex generate
```

From a source checkout:

```sh
cargo run -p rustex-cli -- generate
```

Use `check` in CI to fail when generated files are stale:

```sh
rustex check
```

Use `diff` to see what generation would change:

```sh
rustex diff
```

## Rust Usage

Generated Rust bindings depend on `rustex-runtime`, which wraps the official
Rust `convex` crate.

Typical generated usage:

```rust
use rustex_generated::api::messages;
use rustex_runtime::RustexClient;

let mut client = RustexClient::new(&deployment_url).await?;

let id = rustex_generated::mutation!(client, messages::add, {
    author: "alice",
    body: "hello",
})
.await?;

let messages = rustex_generated::query!(client, messages::collect, {}).await?;
```

The generated function spec ties together:

- the Convex function path
- the argument type
- the output type
- whether the function is a query, mutation, or action

Unsupported or lossy Convex shapes degrade explicitly to `serde_json::Value`
rather than pretending to be fully typed.

## Swift Usage

Generated Swift bindings depend on a bundled `RustexRuntime` target, which wraps
the official `ConvexMobile.ConvexClient`.

Typical generated usage:

```swift
import RustexGenerated

let client = RustexClient(deploymentUrl: deploymentUrl)

let id = try await client.mutation(
  API.Messages.add(author: author, body: body)
)

let messages = try await client.query(API.Messages.collect())

let subscription = client.subscribe(API.Messages.collect())
```

The generated call builders keep Swift call sites close to Convex JavaScript
argument style while still using normal Swift labels and types.

Auth is supported by wrapping the authenticated Convex client:

```swift
let raw = ConvexClientWithAuth(deploymentUrl: deploymentUrl, authProvider: provider)
let client = RustexClient(raw)
```

The Swift runtime exposes:

- `RustexFunctionSpec`
- `RustexQuerySpec`
- `RustexMutationSpec`
- `RustexActionSpec`
- `RustexClient`
- typed `query`, `subscribe`, `mutation`, and `action`
- `watchWebSocketState()`
- `raw` access to the underlying `ConvexClient`

Convex Swift exposes queries through subscriptions, so Rustex implements typed
one-shot `query` by subscribing, resolving the first value, and cancelling.

## Example

The repository includes one shared Convex app and matching Rust and Swift CLIs:

```text
example/
  convex/        # shared Convex schema and functions
  rust/          # Rust CLI using generated Rust bindings
  swift/         # Swift CLI using generated Swift bindings
  rustex.toml    # example Rustex config
```

Regenerate example bindings:

```sh
cargo run -p rustex-cli -- --project example generate
```

Run the Rust example:

```sh
cargo run --manifest-path example/rust/Cargo.toml -- list
cargo run --manifest-path example/rust/Cargo.toml -- add --author alice --body "hello from rust"
cargo run --manifest-path example/rust/Cargo.toml -- watch --updates 1
```

Run the Swift example:

```sh
swift run --package-path example/swift RustexSwiftExample list
swift run --package-path example/swift RustexSwiftExample add --author alice --body "hello from swift"
swift run --package-path example/swift RustexSwiftExample watch --updates 1
```

Both examples read `CONVEX_URL` from `example/.env.local`.

## CLI Commands

```sh
rustex generate
rustex check
rustex diff
rustex inspect functions --format json
rustex watch
rustex init
```

All commands accept `--project <path>` to point at a project directory containing
`rustex.toml`:

```sh
rustex --project example generate
```

## Configuration

Top-level options:

| Option | Meaning |
| --- | --- |
| `project_root` | Project root used for resolving relative paths. |
| `convex_root` | Directory containing the Convex app. |
| `out_dir` | Directory where Rustex writes generated outputs. |
| `emit` | Output targets, such as `rust`, `swift`, `manifest`, `ir`, and `diagnostics`. |
| `strict` | Treat supported diagnostics more strictly. |
| `allow_inferred_returns` | Allow TypeScript checker fallback when a function has no explicit return validator. |
| `naming_strategy` | Naming policy for generated symbols. |
| `id_style` | ID generation policy. |
| `custom_derives` | Additional derives for generated Rust types. |
| `custom_attributes` | Additional attributes for generated Rust types. |

Swift options:

| Option | Default |
| --- | --- |
| `package_name` | `RustexGenerated` |
| `module_name` | `RustexGenerated` |
| `product_name` | `RustexGenerated` |
| `runtime_module_name` | `RustexRuntime` |
| `client_facade_name` | `RustexClient` |
| `generate_package` | `true` |
| `bundle_runtime` | `true` |
| `access_level` | `public` |
| `tools_version` | `5.10` |
| `unknown_type_strategy` | `any_codable` |
| `emit_doc_comments` | `true` |
| `convex_dependency_url` | `https://github.com/get-convex/convex-swift` |
| `convex_dependency_requirement` | `{ kind = "from", version = "0.8.1" }` |

`bundle_runtime = true` is the default so generated Swift output is buildable as
a standalone Swift package without publishing a separate Rustex Swift runtime.

## Type Mapping

Rustex is validator-first: explicit Convex validators are the source of truth
for generated contracts. Common mappings include:

| Convex validator | Rust | Swift |
| --- | --- | --- |
| `v.string()` | `String` | `String` |
| `v.number()` | `f64` | `Double` |
| `v.int64()` | `i64` | `Int64` with Convex wrappers on decoded fields |
| `v.boolean()` | `bool` | `Bool` |
| `v.null()` | `()` or unit-like support types | `RustexNull` or `RustexVoid` |
| `v.bytes()` | `Vec<u8>` | `Data` |
| `v.any()` | `serde_json::Value` | `AnyCodable` |
| `v.id("table")` | `TableId` | `TableId` |
| `v.array(T)` | `Vec<T>` | `[T]` |
| `v.record(v.string(), T)` | `BTreeMap<String, T>` | `[String: T]` |
| `v.object({...})` | generated `struct` | generated `struct` |
| `v.optional(T)` | `Option<T>` | `T?` |
| string literal union | generated enum | raw-value enum |
| discriminated object union | generated enum | custom `Codable` enum |

Unsupported mixed unions and unknown shapes fall back to explicit untyped values.

## Architecture

Workspace crates:

- `crates/rustex-cli`: CLI entrypoint and command orchestration
- `crates/rustex-project`: config loading and project layout resolution
- `crates/rustex-ts-analyzer`: Rust bridge to the Node/TypeScript analyzer
- `crates/rustex-convex`: IR finalization and hashing
- `crates/rustex-ir`: language-agnostic IR model
- `crates/rustex-diagnostics`: structured diagnostics
- `crates/rustex-runtime`: Rust runtime wrapper over the official Convex Rust client
- `crates/rustex-rustgen`: Rust codegen backend
- `crates/rustex-swiftgen`: Swift codegen backend and bundled Swift runtime source generation
- `crates/rustex-output`: deterministic artifact writer
- `crates/rustex-testkit`: shared test utilities
- `packages/ts-analyzer`: TypeScript analyzer bundled for Node

Generation pipeline:

1. Load `rustex.toml`.
2. Resolve the Convex project layout.
3. Run the TypeScript analyzer over schema and function modules.
4. Normalize extracted contracts into Rustex IR.
5. Finalize and hash the IR.
6. Emit configured targets.
7. Write diagnostics and metadata artifacts.

## Development

Run all Rust tests:

```sh
cargo test --workspace
```

Check Swift generation:

```sh
cargo test -p rustex-swiftgen
swift build --package-path example/swift
```

Regenerate the checked-in example output:

```sh
cargo run -p rustex-cli -- --project example generate
```

Work on the TypeScript analyzer:

```sh
cd packages/ts-analyzer
pnpm install
pnpm run check
```

## Current Limits

- Extraction is validator-first; TypeScript inference is a fallback.
- Advanced dynamic validator factories are intentionally not executed.
- Some recursive or highly dynamic validator shapes still fall back to untyped values.
- Generated metadata from `convex/_generated/*.d.ts` is used for topology and corroboration, not full semantic replacement.
- Runtime type safety is only as strong as the contracts Rustex can recover.

