use anyhow::{Context, Result, bail};
use camino::{Utf8Path, Utf8PathBuf};
use clap::{Parser, Subcommand};
use rustex_convex::finalize_ir;
use rustex_output::{write_ir, write_rust};
use rustex_project::load_config;
use rustex_rustgen::generate as generate_rust;
use rustex_ts_analyzer::analyze;
use std::collections::BTreeMap;

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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let project_arg = cli.project.clone();
    let root = canonicalize_utf8(&project_arg)
        .with_context(|| format!("failed to access project root {}", project_arg))?;
    let (config, layout) = load_config(&root)?;
    match cli.command {
        Command::Generate => {
            let package = finalize_ir(analyze(&layout.root, &layout.convex_root)?);
            emit_all(&config.emit, &layout.out_dir, &package)?;
        }
        Command::Check => {
            let package = finalize_ir(analyze(&layout.root, &layout.convex_root)?);
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
            let package = finalize_ir(analyze(&layout.root, &layout.convex_root)?);
            if format == "json" {
                match subject.as_str() {
                    "functions" => {
                        println!("{}", serde_json::to_string_pretty(&package.functions)?)
                    }
                    "tables" => println!("{}", serde_json::to_string_pretty(&package.tables)?),
                    _ => println!("{}", serde_json::to_string_pretty(&package)?),
                }
            } else {
                println!(
                    "tables: {}\nfunctions: {}\ndiagnostics: {}",
                    package.tables.len(),
                    package.functions.len(),
                    package.diagnostics.len()
                );
            }
        }
        Command::Diff => {
            let package = finalize_ir(analyze(&layout.root, &layout.convex_root)?);
            let current = expected_outputs(&config.emit, &layout.out_dir, &package)?;
            for changed in diff_outputs(&current)? {
                println!("{changed}");
            }
        }
    }
    Ok(())
}

fn canonicalize_utf8(path: &Utf8Path) -> Result<Utf8PathBuf> {
    let std_path = std::fs::canonicalize(path)?;
    Utf8PathBuf::from_path_buf(std_path)
        .map_err(|p| anyhow::anyhow!("non-utf8 path: {}", p.display()))
}

fn emit_all(emit: &[String], out_dir: &Utf8Path, package: &rustex_ir::IrPackage) -> Result<()> {
    if emit.iter().any(|e| e == "rust") {
        write_rust(&generate_rust(package)?, &out_dir.join("rust"))?;
    }
    write_ir(package, out_dir)?;
    Ok(())
}

fn expected_outputs(
    emit: &[String],
    out_dir: &Utf8Path,
    package: &rustex_ir::IrPackage,
) -> Result<BTreeMap<Utf8PathBuf, String>> {
    let mut outputs = BTreeMap::new();
    if emit.iter().any(|e| e == "rust") {
        for file in generate_rust(package)? {
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
    Ok(outputs)
}

fn diff_outputs(expected: &BTreeMap<Utf8PathBuf, String>) -> Result<Vec<String>> {
    let mut changed = Vec::new();
    for (path, contents) in expected {
        let current = std::fs::read_to_string(path).ok();
        if current.as_deref() != Some(contents.as_str()) {
            changed.push(path.to_string());
        }
    }
    Ok(changed)
}
