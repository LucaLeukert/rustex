# Architecture Notes

The implemented MVP follows the planned split:

- `rustex-project` loads config and resolves the project layout.
- `rustex-ts-analyzer` owns the subprocess boundary to the TypeScript analyzer.
- `rustex-convex` finalizes and hashes the extracted IR.
- `rustex-ir` defines the normalized cross-language contract model.
- `rustex-rustgen` maps IR into generated Rust source files.
- `rustex-output` writes deterministic artifacts.
- `rustex` exposes `generate`, `check`, `inspect`, and `diff`.

The analyzer intentionally does not execute user code. It walks TypeScript ASTs,
resolves identifiers through the TypeScript checker, and supports a statically
analyzable subset of Convex validators:

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
