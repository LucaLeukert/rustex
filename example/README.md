# example

To install dependencies:

```bash
bun install
```

To run:

```bash
bun run index.ts
```

This project was created using `bun init` in bun v1.3.5. [Bun](https://bun.com) is a fast all-in-one JavaScript runtime.

## Layout

The example keeps one shared Convex app and two typed clients:

- `convex/`: shared Convex schema and functions
- `rust/`: Rust CLI example
- `swift/`: Swift CLI example

Regenerate bindings after changing the Convex schema or functions:

```bash
cargo run -p rustex-cli -- --project . generate
```

This writes Rust and Swift bindings under `convex/_rustex/`.

## Rust CLI

The typed Rust CLI is rooted at [example/rust/Cargo.toml](/Users/lucaleukert/src/rustex/example/rust/Cargo.toml) and talks to the shared Convex deployment through:

- `rustex_runtime::RustexClient` for typed queries, mutations, and subscriptions
- `convex::ConvexClient` for raw subscriptions, decoded back into generated Rust types
- generated `query!`, `mutation!`, and `subscribe!` macros for JS-like call sites

Then run the CLI from the repo root:

```bash
cargo run --manifest-path example/rust/Cargo.toml -- list
cargo run --manifest-path example/rust/Cargo.toml -- add --author alice --body "hello from rust"
cargo run --manifest-path example/rust/Cargo.toml -- watch --updates 1
```

## Swift CLI

The typed Swift CLI is rooted at [example/swift/Package.swift](/Users/lucaleukert/src/rustex/example/swift/Package.swift) and uses the generated Swift package in `convex/_rustex/swift`.

Run it from the repo root:

```bash
swift run --package-path example/swift RustexSwiftExample list
swift run --package-path example/swift RustexSwiftExample add --author alice --body "hello from swift"
swift run --package-path example/swift RustexSwiftExample watch --updates 1
```

`example/.env.local` already contains `CONVEX_URL`, and both CLIs load it automatically.
