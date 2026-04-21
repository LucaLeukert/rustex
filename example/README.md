# Rustex Example

This example contains one Convex app and two generated typed clients:

- `convex/`: shared Convex schema and functions
- `rust/`: Rust CLI using generated Rust bindings
- `swift/`: Swift CLI using generated Swift bindings
- `convex/_rustex/`: generated Rustex output

Both CLIs read the Convex deployment URL from `example/.env.local`:

```sh
CONVEX_URL=https://your-deployment.convex.cloud
```

## Regenerate Bindings

Run from the repository root:

```sh
cargo run -p rustex-cli -- --project example generate
```

The example config enables Rust, Swift, IR, and diagnostics output. Generated
bindings are written under `example/convex/_rustex/`.

## Rust CLI

The Rust CLI is rooted at [example/rust/Cargo.toml](/Users/lucaleukert/src/rustex/example/rust/Cargo.toml).
It uses:

- `rustex_runtime::RustexClient`
- generated Rust models and API specs
- generated `query!`, `mutation!`, and `subscribe!` macros

Run from the repository root:

```sh
cargo run --manifest-path example/rust/Cargo.toml -- list
cargo run --manifest-path example/rust/Cargo.toml -- add --author alice --body "hello from rust"
cargo run --manifest-path example/rust/Cargo.toml -- watch --updates 1
```

## Swift CLI

The Swift CLI is rooted at [example/swift/Package.swift](/Users/lucaleukert/src/rustex/example/swift/Package.swift).
It depends on the generated Swift package in `example/convex/_rustex/swift`
through the local `example/swift/RustexGenerated` symlink.

Run from the repository root:

```sh
swift run --package-path example/swift RustexSwiftExample list
swift run --package-path example/swift RustexSwiftExample add --author alice --body "hello from swift"
swift run --package-path example/swift RustexSwiftExample watch --updates 1
```

The Swift example uses generated call builders:

```swift
let id = try await client.mutation(
  API.Messages.add(author: author, body: body)
)

let messages = try await client.query(API.Messages.collect())
```

