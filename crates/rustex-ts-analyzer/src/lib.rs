use anyhow::{Context, Result, bail};
use camino::Utf8Path;
use rustex_ir::IrPackage;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, process::Command};
use tracing::debug;
use walkdir::WalkDir;

static ANALYZER_BUNDLE: &[u8] = include_bytes!(env!("RUSTEX_TS_ANALYZER_BUNDLE"));
const ANALYZER_BUNDLE_SHA256: &str = env!("RUSTEX_TS_ANALYZER_BUNDLE_SHA256");

pub fn analyze(
    project_root: &Utf8Path,
    convex_root: &Utf8Path,
    allow_inferred_returns: bool,
) -> Result<IrPackage> {
    let _span = tracing::info_span!(
        "rustex_ts_analyzer.analyze",
        project_root = %project_root,
        convex_root = %convex_root,
        allow_inferred_returns
    )
    .entered();
    let script = materialize_analyzer_bundle(project_root)?;
    let node = find_node_binary()?;
    let cache_path = project_root.join(".rustex-cache").join("analyzer.json");
    let cache_key = snapshot_key(project_root, convex_root, allow_inferred_returns)?;

    if let Some(package) = load_cached(&cache_path, &cache_key)? {
        debug!(
            cache_path = %display_path(&cache_path, project_root),
            "using cached analyzer output"
        );
        return Ok(package);
    }

    let mut command = Command::new(node);
    command
        .arg(script.as_os_str())
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
    debug!(
        cache_path = %display_path(&cache_path, project_root),
        "stored analyzer output cache"
    );
    Ok(package)
}

fn materialize_analyzer_bundle(project_root: &Utf8Path) -> Result<PathBuf> {
    let bundle_dir = project_root.join(".rustex-cache").join("runtime");
    fs::create_dir_all(&bundle_dir).with_context(|| {
        format!(
            "failed to create analyzer runtime cache directory {}",
            bundle_dir
        )
    })?;
    let bundle_path = bundle_dir.join(format!("analyze-{ANALYZER_BUNDLE_SHA256}.cjs"));
    let bundle_path_std = bundle_path.as_std_path().to_path_buf();

    let should_write = match fs::read(&bundle_path_std) {
        Ok(existing) => existing != ANALYZER_BUNDLE,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => true,
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to read cached analyzer bundle {}", bundle_path));
        }
    };

    if should_write {
        fs::write(&bundle_path_std, ANALYZER_BUNDLE)
            .with_context(|| format!("failed to write analyzer bundle to {}", bundle_path))?;
    }

    Ok(bundle_path_std)
}

fn find_node_binary() -> Result<PathBuf> {
    if let Ok(explicit) = std::env::var("RUSTEX_NODE_BIN") {
        return verify_node_binary(PathBuf::from(explicit));
    }
    for candidate in ["node", "nodejs"] {
        if let Ok(path) = verify_node_binary(PathBuf::from(candidate)) {
            return Ok(path);
        }
    }
    bail!("failed to locate a usable Node.js binary; set RUSTEX_NODE_BIN or install node");
}

fn verify_node_binary(path: PathBuf) -> Result<PathBuf> {
    let output = Command::new(&path)
        .arg("--version")
        .output()
        .with_context(|| format!("failed to execute Node.js binary {}", path.display()))?;
    if output.status.success() {
        Ok(path)
    } else {
        bail!(
            "Node.js binary {} exited with status {}",
            path.display(),
            output.status
        )
    }
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
) -> Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(project_root.as_str());
    hasher.update(convex_root.as_str());
    hasher.update(if allow_inferred_returns { b"1" } else { b"0" });
    hasher.update(ANALYZER_BUNDLE_SHA256.as_bytes());

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

fn display_path(path: &Utf8Path, project_root: &Utf8Path) -> String {
    path.strip_prefix(project_root)
        .map(Utf8Path::to_string)
        .unwrap_or_else(|_| path.to_string())
}
