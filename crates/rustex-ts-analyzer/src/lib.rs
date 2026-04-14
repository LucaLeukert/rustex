use anyhow::{Context, Result, bail};
use camino::Utf8Path;
use log::info;
use rustex_ir::IrPackage;
use sha2::{Digest, Sha256};
use std::process::Command;
use walkdir::WalkDir;

pub fn analyze(
    project_root: &Utf8Path,
    convex_root: &Utf8Path,
    allow_inferred_returns: bool,
) -> Result<IrPackage> {
    let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = manifest_dir
        .join("../../packages/ts-analyzer/src/analyze.ts")
        .canonicalize_utf8()
        .with_context(|| "failed to resolve analyzer script path")?;
    let cache_path = project_root.join(".rustex-cache").join("analyzer.json");
    let cache_key = snapshot_key(project_root, convex_root, allow_inferred_returns, &script)?;

    if let Some(package) = load_cached(&cache_path, &cache_key)? {
        return Ok(package);
    }

    let mut command = Command::new("bun");
    command
        .arg("run")
        .arg(script.as_str())
        .arg("--project-root")
        .arg(project_root.as_str())
        .arg("--convex-root")
        .arg(convex_root.as_str());
    if allow_inferred_returns {
        command.arg("--allow-inferred-returns");
    }
    let output = command
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
    store_cached(&cache_path, &cache_key, &package)?;
    Ok(package)
}

#[derive(serde::Serialize, serde::Deserialize)]
struct AnalyzerCache {
    key: String,
    package: IrPackage,
}

fn load_cached(cache_path: &Utf8Path, expected_key: &str) -> Result<Option<IrPackage>> {
    let Ok(raw) = std::fs::read_to_string(cache_path) else {
        return Ok(None);
    };
    let cache: AnalyzerCache = serde_json::from_str(&raw)?;
    if cache.key == expected_key {
        Ok(Some(cache.package))
    } else {
        Ok(None)
    }
}

fn store_cached(cache_path: &Utf8Path, key: &str, package: &IrPackage) -> Result<()> {
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let cache = AnalyzerCache {
        key: key.to_string(),
        package: package.clone(),
    };
    std::fs::write(cache_path, serde_json::to_string(&cache)?)?;
    Ok(())
}

fn snapshot_key(
    project_root: &Utf8Path,
    convex_root: &Utf8Path,
    allow_inferred_returns: bool,
    script_path: &Utf8Path,
) -> Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(project_root.as_str());
    hasher.update(convex_root.as_str());
    hasher.update(if allow_inferred_returns { b"1" } else { b"0" });
    if let Ok(bytes) = std::fs::read(script_path) {
        hasher.update(bytes);
    }

    for entry in WalkDir::new(convex_root)
        .sort_by_file_name()
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        let metadata = entry.metadata()?;
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update(metadata.len().to_le_bytes());
        if let Ok(modified) = metadata.modified() {
            if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                hasher.update(duration.as_secs().to_le_bytes());
                hasher.update(duration.subsec_nanos().to_le_bytes());
            }
        }
    }

    let config_path = project_root.join("rustex.toml");
    if let Ok(bytes) = std::fs::read(&config_path) {
        hasher.update(bytes);
    }

    Ok(format!("{:x}", hasher.finalize()))
}
