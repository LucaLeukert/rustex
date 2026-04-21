# Architecture Notes

The implemented MVP follows the planned split:

- `rustex-project` loads config and resolves the project layout.
- `rustex-ts-analyzer` owns the subprocess boundary to the bundled Node/TypeScript analyzer.
- `rustex-convex` finalizes and hashes the extracted IR.
- `rustex-ir` defines the normalized cross-language contract model.
- `rustex-rustgen` maps IR into generated Rust source files.
- `rustex-swiftgen` maps IR into a Swift Package with generated bindings and
  bundled Swift runtime sources.
- `rustex-output` writes deterministic artifacts.
- `rustex` exposes `generate`, `check`, `inspect`, and `diff`.

The analyzer intentionally does not execute user code. It is authored in
TypeScript with `effect` for typed control flow and failure handling, bundled
to a single JavaScript file as a build step, embedded into the Rust crate, and
executed with Node.js,
walks TypeScript ASTs, resolves identifiers through the TypeScript checker, and
supports a statically analyzable subset of Convex validators:

- `v.string`
- `v.number`
- `v.int64`
- `v.boolean`
- `v.null`
- `v.bytes`
- `v.any`
- `v.literal`
- `v.id`
- `v.array`
- `v.record`
- `v.object`
- `v.union`
- `v.optional`

Unsupported or lossy surfaces are emitted as diagnostics rather than silently
invented as precise contracts.

Tests synthesize temporary fixture projects from the repository's root `convex/`
example instead of storing duplicate committed Convex app copies.

## Swift target

Swift generation is selected with `emit = ["swift"]` in `rustex.toml`. The
backend writes `<out_dir>/swift` and, by default, generates a complete Swift
Package with two targets:

- `RustexRuntime`: a Swift runtime over the official `ConvexMobile` package.
- `RustexGenerated`: app-specific ids, document models, function args,
  responses, and API specs.

The Swift runtime follows the same contract shape as `rustex-runtime`:

- `RustexFunctionSpec`, `RustexQuerySpec`, `RustexMutationSpec`, and
  `RustexActionSpec`
- `RustexClient` wrapping `ConvexClient`
- typed `query`, `subscribe`, `mutation`, and `action`
- argument encoding into `[String: ConvexEncodable?]`
- response decoding through Convex Swift's `Decodable` path
- structured `RustexRuntimeError`
- `watchWebSocketState()` for the websocket-state surface Convex Swift exposes

Convex Swift does not expose a direct equivalent to the Rust client's
`watch_all`, so the runtime keeps the underlying `ConvexClient` available
through `RustexClient.raw` for feature parity with the official Swift SDK.
