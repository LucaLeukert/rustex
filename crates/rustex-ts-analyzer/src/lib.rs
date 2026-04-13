use anyhow::{Context, Result, bail};
use camino::Utf8Path;
use rustex_ir::IrPackage;
use std::process::Command;

pub fn analyze(project_root: &Utf8Path, convex_root: &Utf8Path) -> Result<IrPackage> {
    let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = manifest_dir
        .join("../../packages/ts-analyzer/src/analyze.mjs")
        .canonicalize_utf8()
        .with_context(|| "failed to resolve analyzer script path")?;
    let output = Command::new("node")
        .arg(script.as_str())
        .arg("--project-root")
        .arg(project_root.as_str())
        .arg("--convex-root")
        .arg(convex_root.as_str())
        .output()
        .with_context(|| "failed to spawn Node analyzer")?;

    if !output.status.success() {
        bail!(
            "analyzer failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let mut package: IrPackage = serde_json::from_slice(&output.stdout)
        .with_context(|| "failed to parse analyzer output")?;
    package.project.root = project_root.to_path_buf();
    package.project.convex_root = convex_root.to_path_buf();
    Ok(package)
}
