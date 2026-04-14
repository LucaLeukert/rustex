use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn generates_outputs_for_basic_fixture() -> Result<()> {
    let temp = fixture_project()?;

    let status = Command::new(env!("CARGO_BIN_EXE_rustex"))
        .arg("--project")
        .arg(&temp)
        .arg("generate")
        .status()?;
    assert!(status.success());

    let ir = fs::read_to_string(temp.join("generated/rustex/rustex.ir.json"))?;
    assert!(ir.contains("\"canonical_path\": \"messages:add\""));

    let models = fs::read_to_string(temp.join("generated/rustex/rust/models.rs"))?;
    assert!(models.contains("pub struct MessagesDoc"));
    assert!(models.contains("pub author: String"));

    let api = fs::read_to_string(temp.join("generated/rustex/rust/api.rs"))?;
    assert!(api.contains("pub mod messages"));
    assert!(api.contains("impl MutationSpec for Add"));
    assert!(api.contains("const PATH: &'static str = \"messages:add\""));

    Ok(())
}

#[test]
fn generated_files_match_golden_output() -> Result<()> {
    let temp = fixture_project()?;

    let status = Command::new(env!("CARGO_BIN_EXE_rustex"))
        .arg("--project")
        .arg(&temp)
        .arg("generate")
        .status()?;
    assert!(status.success());

    let api = fs::read_to_string(temp.join("generated/rustex/rust/api.rs"))?;
    assert_eq!(normalize_golden(&api), normalize_golden(include_str!("golden/api.rs")));

    let models = fs::read_to_string(temp.join("generated/rustex/rust/models.rs"))?;
    assert_eq!(
        normalize_golden(&models),
        normalize_golden(include_str!("golden/models.rs"))
    );

    Ok(())
}

#[test]
fn init_scaffolds_default_config() -> Result<()> {
    let root = workspace_root();
    let temp = unique_temp_dir()?;
    copy_dir(&root.join("convex"), &temp.join("convex"))?;

    let status = Command::new(env!("CARGO_BIN_EXE_rustex"))
        .arg("--project")
        .arg(&temp)
        .arg("init")
        .status()?;
    assert!(status.success());

    let config = fs::read_to_string(temp.join("rustex.toml"))?;
    assert!(config.contains("project_root = \".\""));
    assert!(config.contains("convex_root = \"./convex\""));
    assert!(config.contains("out_dir = \"./generated/rustex\""));

    Ok(())
}

#[test]
fn advanced_fixture_generates_named_types_and_compiles() -> Result<()> {
    let temp = advanced_fixture_project()?;

    let status = Command::new(env!("CARGO_BIN_EXE_rustex"))
        .arg("--project")
        .arg(&temp)
        .arg("generate")
        .status()?;
    assert!(status.success());

    let models = fs::read_to_string(temp.join("generated/rustex/rust/models.rs"))?;
    assert!(models.contains("pub enum MessagesDocState"));
    assert!(models.contains("pub struct MessagesDocMetadata"));
    assert!(models.contains("pub enum MessagesDocAttachment"));

    let api = fs::read_to_string(temp.join("generated/rustex/rust/api.rs"))?;
    assert!(api.contains("pub struct SendArgsMessage"));
    assert!(api.contains("pub enum SendResponseDelivery"));
    assert!(api.contains("pub struct ListResponseItem"));

    let source_map = fs::read_to_string(temp.join("generated/rustex/rustex.source_map.json"))?;
    assert!(source_map.contains("generated_symbol"));

    let schema = fs::read_to_string(temp.join("generated/rustex/rustex.schema.json"))?;
    assert!(schema.contains("\"$defs\""));

    let openapi = fs::read_to_string(temp.join("generated/rustex/rustex.openapi.json"))?;
    assert!(openapi.contains("\"openapi\": \"3.1.0\""));

    let check = Command::new("cargo")
        .arg("check")
        .arg("--manifest-path")
        .arg(temp.join("generated/rustex/rust/Cargo.toml"))
        .status()?;
    assert!(check.success());

    Ok(())
}

#[test]
fn helper_diagnostics_include_snippets_in_text_output() -> Result<()> {
    let temp = helper_fixture_project()?;

    let inspect = Command::new(env!("CARGO_BIN_EXE_rustex"))
        .arg("--project")
        .arg(&temp)
        .arg("inspect")
        .arg("diagnostics")
        .output()?;
    assert!(inspect.status.success());

    let stdout = String::from_utf8(inspect.stdout)?;
    assert!(stdout.contains("RX042"));
    assert!(stdout.contains("metadata"));
    assert!(stdout.contains("metadata,"));

    Ok(())
}

#[test]
fn monorepo_auto_discovery_and_component_metadata_work() -> Result<()> {
    let temp = unique_temp_dir()?;
    fs::create_dir_all(temp.join("apps/chat/convex/components/presence"))?;
    fs::write(
        temp.join("rustex.toml"),
        r#"project_root = "."
convex_root = "./convex"
out_dir = "./generated/rustex"
emit = ["rust", "manifest", "ir"]
strict = false
allow_inferred_returns = false
naming_strategy = "safe"
id_style = "newtype_per_table"
"#,
    )?;
    fs::write(
        temp.join("apps/chat/convex/schema.ts"),
        r#"import { defineSchema, defineTable } from "convex/server";
import { v } from "convex/values";
export default defineSchema({ messages: defineTable({ body: v.string() }) });
"#,
    )?;
    fs::write(
        temp.join("apps/chat/convex/components/presence/ping.ts"),
        r#"import { query } from "../../_generated/server";
import { v } from "convex/values";
export const ping = query({ args: {}, returns: v.object({ ok: v.boolean() }), handler: async () => ({ ok: true }) });
"#,
    )?;
    fs::create_dir_all(temp.join("apps/chat/convex/_generated"))?;
    fs::write(temp.join("apps/chat/convex/_generated/api.d.ts"), "import type * as ping from \"../components/presence/ping\";")?;
    fs::write(temp.join("apps/chat/convex/_generated/server.d.ts"), "export declare const query: any;")?;
    fs::write(temp.join("apps/chat/convex/_generated/dataModel.d.ts"), "export type Id<TableName extends string> = string;")?;

    let status = Command::new(env!("CARGO_BIN_EXE_rustex"))
        .arg("--project")
        .arg(&temp)
        .arg("generate")
        .status()?;
    assert!(status.success());

    let ir = fs::read_to_string(temp.join("generated/rustex/rustex.ir.json"))?;
    assert!(ir.contains("\"component_path\": \"components/presence\""));
    assert!(ir.contains("\"discovered_convex_roots\""));
    assert!(ir.contains("\"component_roots\""));

    Ok(())
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn fixture_project() -> Result<PathBuf> {
    let temp = unique_temp_dir()?;
    fs::create_dir_all(temp.join("convex/_generated"))?;
    fs::write(
        temp.join("convex/schema.ts"),
        r#"import { defineSchema, defineTable } from "convex/server";
import { v } from "convex/values";

export default defineSchema({
  messages: defineTable({
    author: v.string(),
    body: v.string(),
  }),
});
"#,
    )?;
    fs::write(
        temp.join("convex/messages.ts"),
        r#"import { mutation, query } from "./_generated/server";
import { v } from "convex/values";

export const add = mutation({
  args: { author: v.string(), body: v.string() },
  handler: async (_ctx, _args) => {
    return null;
  },
});

export const collect = query({
  handler: async (_ctx) => {
    return [];
  },
});
"#,
    )?;
    fs::write(
        temp.join("convex/_generated/api.d.ts"),
        r#"import type * as messages from "../messages";
declare const fullApi: { messages: typeof messages };
export declare const api: typeof fullApi;
"#,
    )?;
    fs::write(
        temp.join("convex/_generated/server.d.ts"),
        r#"export declare const query: any;
export declare const mutation: any;
export declare const action: any;
export declare const internalQuery: any;
export declare const internalMutation: any;
export declare const internalAction: any;
export declare const httpAction: any;
"#,
    )?;
    fs::write(
        temp.join("convex/_generated/dataModel.d.ts"),
        r#"export type Id<TableName extends string> = string;
"#,
    )?;
    fs::write(temp.join("rustex.toml"), fixture_config())?;
    Ok(temp)
}

fn advanced_fixture_project() -> Result<PathBuf> {
    let temp = unique_temp_dir()?;
    fs::create_dir_all(temp.join("convex/_generated"))?;
    fs::write(temp.join("rustex.toml"), advanced_fixture_config())?;
    fs::write(temp.join("convex/schema.ts"), advanced_schema())?;
    fs::write(temp.join("convex/messages.ts"), advanced_messages())?;
    fs::write(
        temp.join("convex/_generated/api.d.ts"),
        r#"import type * as messages from "../messages";
declare const fullApi: { messages: typeof messages };
export declare const api: typeof fullApi;
"#,
    )?;
    fs::write(
        temp.join("convex/_generated/server.d.ts"),
        r#"export declare const query: any;
export declare const mutation: any;
export declare const action: any;
export declare const internalQuery: any;
export declare const internalMutation: any;
export declare const internalAction: any;
export declare const httpAction: any;
"#,
    )?;
    fs::write(
        temp.join("convex/_generated/dataModel.d.ts"),
        r#"export type Id<TableName extends string> = string & { __tableName: TableName };
"#,
    )?;
    Ok(temp)
}

fn helper_fixture_project() -> Result<PathBuf> {
    let temp = unique_temp_dir()?;
    let root = workspace_root();
    copy_dir(&root.join("convex"), &temp.join("convex"))?;
    fs::write(temp.join("rustex.toml"), fixture_config())?;
    fs::write(
        temp.join("convex/schema.ts"),
        r#"import { defineSchema, defineTable } from "convex/server";
import { v } from "convex/values";

const metadata = {
  tags: v.array(v.string()),
};

export default defineSchema({
  messages: defineTable({
    author: v.string(),
    metadata,
  }),
});
"#,
    )?;
    Ok(temp)
}

fn unique_temp_dir() -> Result<PathBuf> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?;
    for attempt in 0..100 {
        let temp = std::env::temp_dir().join(format!(
            "rustex-fixture-{}-{}-{}-{}",
            std::process::id(),
            now.as_secs(),
            now.subsec_nanos(),
            attempt
        ));
        match fs::create_dir(&temp) {
            Ok(()) => return Ok(temp),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    anyhow::bail!("failed to allocate a unique temp fixture directory")
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let path = entry.path();
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&path, &target)?;
        } else {
            fs::copy(path, target)?;
        }
    }
    Ok(())
}

fn fixture_config() -> &'static str {
    r#"project_root = "."
convex_root = "./convex"
out_dir = "./generated/rustex"
emit = ["rust", "manifest", "ir"]
strict = false
allow_inferred_returns = false
naming_strategy = "safe"
id_style = "newtype_per_table"
"#
}

fn advanced_fixture_config() -> &'static str {
    r#"project_root = "."
convex_root = "./convex"
out_dir = "./generated/rustex"
emit = ["rust", "manifest", "ir"]
strict = false
allow_inferred_returns = true
naming_strategy = "safe"
id_style = "newtype_per_table"
"#
}

fn advanced_schema() -> &'static str {
    r#"import { defineSchema, defineTable } from "convex/server";
import { v } from "convex/values";

export default defineSchema({
  messages: defineTable({
    body: v.string(),
    metadata: v.object({
      kind: v.union(v.literal("plain"), v.literal("rich")),
      tags: v.array(v.string()),
      author: v.object({
        name: v.string(),
        role: v.optional(v.union(v.literal("admin"), v.literal("member"))),
      }),
    }),
    state: v.union(v.literal("draft"), v.literal("sent")),
    attachment: v.union(
      v.object({
        kind: v.literal("image"),
        url: v.string(),
        width: v.number(),
      }),
      v.object({
        kind: v.literal("file"),
        name: v.string(),
        size: v.number(),
      }),
    ),
  }),
});
"#
}

fn advanced_messages() -> &'static str {
    r#"import { mutation, query } from "./_generated/server";
import { v } from "convex/values";

const sharedArgs = {
  channelId: v.string(),
};

export const send = mutation({
  args: {
    ...sharedArgs,
    message: v.object({
      text: v.string(),
      meta: v.object({
        urgent: v.optional(v.boolean()),
      }),
    }),
  },
  returns: v.object({
    ok: v.boolean(),
    delivery: v.union(
      v.object({
        kind: v.literal("queued"),
        etaMs: v.number(),
      }),
      v.object({
        kind: v.literal("sent"),
        id: v.id("messages"),
      }),
    ),
  }),
  handler: async (_ctx, _args) => {
    return { ok: true, delivery: { kind: "queued", etaMs: 1 } };
  },
});

export const list = query({
  handler: async (_ctx) => {
    return [
      {
        kind: "sent" as const,
        messages: [
          {
            body: "hi",
            metadata: {
              kind: "plain" as const,
              tags: ["a"],
              author: {
                name: "alice",
                role: "member" as const,
              },
            },
            state: "sent" as const,
            attachment: {
              kind: "image" as const,
              url: "https://example.com/image.png",
              width: 2,
            },
          },
        ],
      },
    ];
  },
});
"#
}

fn normalize_golden(contents: &str) -> String {
    contents
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end()
        .to_string()
}
