use anyhow::{Context, Result, bail};
use camino::{Utf8Path, Utf8PathBuf};
use clap::{Parser, Subcommand};
use rustex_convex::finalize_ir;
use rustex_output::{
    json_schema_document, openapi_document, source_map_document, write_diagnostics, write_ir,
    write_json_schema, write_manifest, write_openapi, write_rust, write_source_map,
};
use rustex_project::{RustexConfig, load_config};
use rustex_rustgen::generate as generate_rust;
use rustex_ts_analyzer::analyze;
use std::collections::BTreeMap;
use std::thread;
use std::time::{Duration, SystemTime};
use tracing::{debug, info, warn};
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Parser)]
#[command(author, version, about = "Convex -> Rust code generation toolkit")]
struct Cli {
    #[arg(long, default_value = ".")]
    project: Utf8PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Generate,
    Check,
    Inspect {
        #[arg(default_value = "summary")]
        subject: String,
        #[arg(long, default_value = "text")]
        format: String,
    },
    Diff,
    Watch {
        #[arg(long, default_value_t = 500)]
        poll_ms: u64,
    },
    Init {
        #[arg(long)]
        force: bool,
    },
}

fn main() -> Result<()> {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init()
        .map_err(|error| anyhow::anyhow!("failed to initialize tracing subscriber: {error}"))?;
    let cli = Cli::parse();
    let project_arg = cli.project.clone();
    match cli.command {
        Command::Init { force } => init_project(&project_arg, force)?,
        other => {
            let root = canonicalize_or_current_utf8(&project_arg)
                .with_context(|| format!("failed to access project root {}", project_arg))?;
            run_command(&root, other)?;
        }
    }
    Ok(())
}

fn canonicalize_utf8(path: &Utf8Path) -> Result<Utf8PathBuf> {
    let std_path = std::fs::canonicalize(path)?;
    Utf8PathBuf::from_path_buf(std_path)
        .map_err(|p| anyhow::anyhow!("non-utf8 path: {}", p.display()))
}

fn canonicalize_or_current_utf8(path: &Utf8Path) -> Result<Utf8PathBuf> {
    if path.exists() {
        canonicalize_utf8(path)
    } else if path == Utf8Path::new(".") {
        let current = std::env::current_dir()?;
        Utf8PathBuf::from_path_buf(current)
            .map_err(|p| anyhow::anyhow!("non-utf8 path: {}", p.display()))
    } else {
        bail!("project root does not exist: {}", path);
    }
}

fn run_command(root: &Utf8Path, command: Command) -> Result<()> {
    match command {
        Command::Generate => {
            info!("starting generation for project at {}", root);
            let (config, layout) = load_config(root)?;
            emit_generate(&config, &layout)?;
        }
        Command::Check => {
            info!("checking generated outputs for project at {}", root);
            let (config, layout) = load_config(root)?;
            let package = analyze_package(&layout)?;
            if package
                .diagnostics
                .iter()
                .any(|d| matches!(d.severity, rustex_diagnostics::Severity::Error))
            {
                bail!("analysis contains blocking errors");
            }
            let current = expected_outputs(&config.emit, &layout.out_dir, &package)?;
            let changed = diff_outputs(&current)?;
            if !changed.is_empty() {
                bail!("generated outputs are stale: {}", changed.join(", "));
            }
        }
        Command::Inspect { subject, format } => {
            info!("inspecting {} for project at {}", subject, root);
            let (_, layout) = load_config(root)?;
            let package = analyze_package(&layout)?;
            if format == "json" {
                match subject.as_str() {
                    "functions" => {
                        println!("{}", serde_json::to_string_pretty(&package.functions)?)
                    }
                    "tables" => println!("{}", serde_json::to_string_pretty(&package.tables)?),
                    _ => println!("{}", serde_json::to_string_pretty(&package)?),
                }
            } else {
                println!("{}", render_inspect_text(&package, &subject));
            }
        }
        Command::Diff => {
            info!("diffing generated outputs for project at {}", root);
            let (config, layout) = load_config(root)?;
            let package = analyze_package(&layout)?;
            let current = expected_outputs(&config.emit, &layout.out_dir, &package)?;
            for changed in diff_outputs(&current)? {
                info!("change detected in {}", changed);
            }
        }
        Command::Watch { poll_ms } => {
            info!("watching {} every {}ms", root, poll_ms);
            watch(root, poll_ms)?;
        }
        Command::Init { .. } => unreachable!("handled before project loading"),
    }

    Ok(())
}

fn analyze_package(layout: &rustex_project::ProjectLayout) -> Result<rustex_ir::IrPackage> {
    let _span = tracing::info_span!(
        "rustex_cli.analyze_package",
        project_root = %layout.root,
        convex_root = %layout.convex_root
    )
    .entered();
    let (config, _) = load_config(&layout.root)?;
    let mut package = analyze(
        &layout.root,
        &layout.convex_root,
        config.allow_inferred_returns,
    )?;
    package.project.discovered_convex_roots = layout.discovered_convex_roots.clone();
    package.project.component_roots = layout.component_roots.clone();
    Ok(finalize_ir(package))
}

fn emit_generate(config: &RustexConfig, layout: &rustex_project::ProjectLayout) -> Result<()> {
    let _span =
        tracing::info_span!("rustex_cli.emit_generate", out_dir = %layout.out_dir).entered();
    let package = analyze_package(layout)?;
    emit_all(&config.emit, &layout.out_dir, &package)
}

fn emit_all(emit: &[String], out_dir: &Utf8Path, package: &rustex_ir::IrPackage) -> Result<()> {
    for e in emit {
        let _span =
            tracing::debug_span!("rustex_cli.emit", artifact = %e, out_dir = %out_dir).entered();
        debug!("emitting artifact");
        match e.as_str() {
            "rust" => {
                let (config, _) = load_config(&package.project.root)?;
                let files = generate_rust(package, &config)?;
                write_rust(&files, &out_dir.join("rust"))?;
            }
            "ir" => write_ir(package, out_dir)?,
            "manifest" => write_manifest(package, out_dir)?,
            "diagnostics" => write_diagnostics(package, out_dir)?,
            "source_map" => write_source_map(package, out_dir)?,
            "schema" => write_json_schema(package, out_dir)?,
            "openapi" => write_openapi(package, out_dir)?,
            other => warn!(emit = %other, "unknown emit type"),
        }
        debug!("finished emitting artifact");
    }
    Ok(())
}

fn expected_outputs(
    emit: &[String],
    out_dir: &Utf8Path,
    package: &rustex_ir::IrPackage,
) -> Result<BTreeMap<Utf8PathBuf, String>> {
    let mut outputs = BTreeMap::new();
    let (config, _) = load_config(&package.project.root)?;
    if emit.iter().any(|e| e == "rust") {
        for file in generate_rust(package, &config)? {
            outputs.insert(out_dir.join("rust").join(&file.path), file.contents);
        }
    }
    outputs.insert(
        out_dir.join("rustex.ir.json"),
        serde_json::to_string_pretty(package)?,
    );
    outputs.insert(
        out_dir.join("rustex.manifest.json"),
        serde_json::to_string_pretty(&package.manifest_meta)?,
    );
    outputs.insert(
        out_dir.join("rustex.diagnostics.json"),
        serde_json::to_string_pretty(&package.diagnostics)?,
    );
    outputs.insert(
        out_dir.join("rustex.source_map.json"),
        serde_json::to_string_pretty(&source_map_document(package))?,
    );
    outputs.insert(
        out_dir.join("rustex.schema.json"),
        serde_json::to_string_pretty(&json_schema_document(package))?,
    );
    outputs.insert(
        out_dir.join("rustex.openapi.json"),
        serde_json::to_string_pretty(&openapi_document(package))?,
    );
    Ok(outputs)
}

fn diff_outputs(expected: &BTreeMap<Utf8PathBuf, String>) -> Result<Vec<String>> {
    let _span =
        tracing::debug_span!("rustex_cli.diff_outputs", file_count = expected.len()).entered();
    let mut changed = Vec::new();
    for (path, contents) in expected {
        let current = std::fs::read_to_string(path).ok();
        if current.as_deref() != Some(contents.as_str()) {
            changed.push(path.to_string());
        }
    }
    Ok(changed)
}

fn render_inspect_text(package: &rustex_ir::IrPackage, subject: &str) -> String {
    match subject {
        "functions" => package
            .functions
            .iter()
            .map(|function| {
                format!(
                    "{} {:?} {:?}",
                    function.canonical_path, function.visibility, function.kind
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        "tables" => package
            .tables
            .iter()
            .map(|table| format!("{} -> {}", table.name, table.doc_name))
            .collect::<Vec<_>>()
            .join("\n"),
        "diagnostics" => render_diagnostics_text(&package.diagnostics),
        _ => {
            let mut lines = vec![
                format!("tables: {}", package.tables.len()),
                format!("functions: {}", package.functions.len()),
                format!("diagnostics: {}", package.diagnostics.len()),
            ];
            if !package.diagnostics.is_empty() {
                lines.push(String::new());
                lines.push(render_diagnostics_text(&package.diagnostics));
            }
            lines.join("\n")
        }
    }
}

fn render_diagnostics_text(diagnostics: &[rustex_diagnostics::Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| {
            let span = diagnostic
                .primary_span
                .as_ref()
                .map(|span| format!("{}:{}:{}", span.file, span.line, span.column))
                .unwrap_or_else(|| "<unknown>".into());
            let mut rendered = format!(
                "[{} {:?}] {} at {}",
                diagnostic.code, diagnostic.severity, diagnostic.message, span
            );
            if let Some(snippet) = &diagnostic.snippet {
                rendered.push('\n');
                rendered.push_str(snippet);
            }
            rendered
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn init_project(project: &Utf8Path, force: bool) -> Result<()> {
    let root = if project.exists() {
        canonicalize_utf8(project)?
    } else if project.is_relative() {
        let joined = std::env::current_dir()?.join(project.as_std_path());
        std::fs::create_dir_all(&joined)?;
        Utf8PathBuf::from_path_buf(joined.canonicalize()?)
            .map_err(|p| anyhow::anyhow!("non-utf8 path: {}", p.display()))?
    } else {
        std::fs::create_dir_all(project)?;
        canonicalize_utf8(project)?
    };

    if !root.is_dir() {
        bail!("project root is not a directory: {}", root);
    }

    let config_path = root.join("rustex.toml");
    if config_path.exists() && !force {
        bail!(
            "config already exists at {}. re-run with --force to overwrite it",
            config_path
        );
    }

    let convex_root = detect_convex_root(&root)
        .ok_or_else(|| anyhow::anyhow!("could not find a convex/ directory under {}", root))?;
    let relative_convex = relative_to_root(&root, &convex_root);
    let config = RustexConfig {
        project_root: Utf8PathBuf::from("."),
        convex_root: relative_convex,
        out_dir: Utf8PathBuf::from("./generated/rustex"),
        ..RustexConfig::default()
    };
    std::fs::write(&config_path, toml::to_string_pretty(&config)?)?;
    println!("initialized {}", config_path);
    Ok(())
}

fn detect_convex_root(root: &Utf8Path) -> Option<Utf8PathBuf> {
    let direct = root.join("convex");
    if direct.is_dir() {
        return Some(direct);
    }

    let app_convex = root.join("app").join("convex");
    if app_convex.is_dir() {
        return Some(app_convex);
    }

    None
}

fn relative_to_root(root: &Utf8Path, path: &Utf8Path) -> Utf8PathBuf {
    path.strip_prefix(root)
        .map(|relative| Utf8PathBuf::from("./").join(relative))
        .unwrap_or_else(|_| path.to_path_buf())
}

fn watch(root: &Utf8Path, poll_ms: u64) -> Result<()> {
    let mut previous = snapshot_inputs(root)?;
    let (config, layout) = load_config(root)?;
    emit_generate(&config, &layout)?;

    loop {
        thread::sleep(Duration::from_millis(poll_ms.max(50)));
        let current = snapshot_inputs(root)?;
        if current == previous {
            continue;
        }

        previous = current;
        info!("change detected, regenerating");
        match load_config(root).and_then(|(config, layout)| emit_generate(&config, &layout)) {
            Ok(()) => info!("generation completed successfully"),
            Err(error) => tracing::error!(error = ?error, "generation failed"),
        }
    }
}

fn snapshot_inputs(root: &Utf8Path) -> Result<BTreeMap<Utf8PathBuf, SystemTime>> {
    let (_, layout) = load_config(root)?;
    let mut entries = BTreeMap::new();
    collect_snapshot_path(&layout.config_path, &mut entries)?;
    collect_snapshot_path(&layout.convex_root, &mut entries)?;
    Ok(entries)
}

fn collect_snapshot_path(
    path: &Utf8Path,
    entries: &mut BTreeMap<Utf8PathBuf, SystemTime>,
) -> Result<()> {
    let metadata = std::fs::metadata(path)?;
    entries.insert(path.to_path_buf(), metadata.modified()?);
    if metadata.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let child = Utf8PathBuf::from_path_buf(entry.path())
                .map_err(|p| anyhow::anyhow!("non-utf8 path: {}", p.display()))?;
            collect_snapshot_path(&child, entries)?;
        }
    }
    Ok(())
}
