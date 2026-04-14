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

## Rust CLI

This example also includes a typed Rust CLI in [example/rust-cli](/Users/lucaleukert/src/rustex/example/rust-cli/Cargo.toml) that talks to the same Convex deployment through:

- `rustex_runtime::TypedConvexClient` for typed queries and mutations
- `convex::ConvexClient` for raw subscriptions, decoded back into generated Rust types

The CLI depends on the generated Rustex crate in [example/convex/_rustex/rust](/Users/lucaleukert/src/rustex/example/convex/_rustex/rust/Cargo.toml), so regenerate after changing the Convex schema or functions:

```bash
cargo run -p rustex -- --project . generate
```

Then run the CLI from the repo root:

```bash
cargo run --manifest-path example/rust-cli/Cargo.toml -- list
cargo run --manifest-path example/rust-cli/Cargo.toml -- add --author alice --body "hello from rust"
cargo run --manifest-path example/rust-cli/Cargo.toml -- find --author alice
cargo run --manifest-path example/rust-cli/Cargo.toml -- status
cargo run --manifest-path example/rust-cli/Cargo.toml -- watch --updates 1
```

`example/.env.local` already contains `CONVEX_URL`, and the CLI loads it automatically.
