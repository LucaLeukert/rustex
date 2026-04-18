use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use tracing::debug;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RustexConfig {
    pub project_root: Utf8PathBuf,
    pub convex_root: Utf8PathBuf,
    pub out_dir: Utf8PathBuf,
    #[serde(default = "default_emit")]
    pub emit: Vec<String>,
    #[serde(default)]
    pub strict: bool,
    #[serde(default)]
    pub allow_inferred_returns: bool,
    #[serde(default = "default_naming")]
    pub naming_strategy: String,
    #[serde(default = "default_id_style")]
    pub id_style: String,
    #[serde(default)]
    pub custom_derives: Vec<String>,
    #[serde(default)]
    pub custom_attributes: Vec<String>,
}

fn default_emit() -> Vec<String> {
    vec!["rust".into(), "manifest".into(), "ir".into()]
}

fn default_naming() -> String {
    "safe".into()
}

fn default_id_style() -> String {
    "newtype_per_table".into()
}

impl Default for RustexConfig {
    fn default() -> Self {
        Self {
            project_root: Utf8PathBuf::from("."),
            convex_root: Utf8PathBuf::from("./convex"),
            out_dir: Utf8PathBuf::from("./generated/rustex"),
            emit: default_emit(),
            strict: false,
            allow_inferred_returns: true,
            naming_strategy: default_naming(),
            id_style: default_id_style(),
            custom_derives: Vec::new(),
            custom_attributes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectLayout {
    pub root: Utf8PathBuf,
    pub convex_root: Utf8PathBuf,
    pub out_dir: Utf8PathBuf,
    pub config_path: Utf8PathBuf,
    pub discovered_convex_roots: Vec<Utf8PathBuf>,
    pub component_roots: Vec<Utf8PathBuf>,
}

pub fn load_config(root: &Utf8Path) -> Result<(RustexConfig, ProjectLayout)> {
    let _span = tracing::info_span!("rustex_project.load_config", root = %root).entered();
    let config_path = root.join("rustex.toml");
    let raw = std::fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read config at {config_path}"))?;
    let config: RustexConfig =
        toml::from_str(&raw).with_context(|| format!("failed to parse {config_path}"))?;

    if !root.exists() {
        anyhow::bail!("project root does not exist: {root}");
    }

    let discovered_convex_roots = discover_convex_roots(root);
    let configured_convex_root = absolutize(root, &config.convex_root);
    let convex_root = if configured_convex_root.exists() {
        configured_convex_root
    } else if discovered_convex_roots.len() == 1 {
        discovered_convex_roots[0].clone()
    } else {
        configured_convex_root
    };

    let layout = ProjectLayout {
        root: root.to_path_buf(),
        convex_root: convex_root.clone(),
        out_dir: absolutize(root, &config.out_dir),
        config_path,
        component_roots: discover_component_roots(&convex_root),
        discovered_convex_roots,
    };

    validate_layout(&layout)?;
    debug!(
        convex_root = %display_path(&layout.convex_root, &layout.root),
        out_dir = %display_path(&layout.out_dir, &layout.root),
        "resolved project layout"
    );

    Ok((config, layout))
}

fn absolutize(root: &Utf8Path, path: &Utf8Path) -> Utf8PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn display_path(path: &Utf8Path, root: &Utf8Path) -> String {
    path.strip_prefix(root)
        .map(Utf8Path::to_string)
        .unwrap_or_else(|_| path.to_string())
}

fn validate_layout(layout: &ProjectLayout) -> Result<()> {
    if !layout.root.is_dir() {
        anyhow::bail!("project root is not a directory: {}", layout.root);
    }

    if !layout.config_path.is_file() {
        anyhow::bail!("missing rustex config: {}", layout.config_path);
    }

    if !layout.convex_root.exists() {
        let candidates = if layout.discovered_convex_roots.is_empty() {
            String::new()
        } else {
            format!(
                " discovered candidates: {}",
                layout
                    .discovered_convex_roots
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        anyhow::bail!(
            "convex root does not exist: {}. rustex supports standard convex/ layouts and can auto-detect common monorepo locations.{}",
            layout.convex_root,
            candidates
        );
    }

    if !layout.convex_root.is_dir() {
        anyhow::bail!("convex root is not a directory: {}", layout.convex_root);
    }

    let schema_path = layout.convex_root.join("schema.ts");
    let generated_dir = layout.convex_root.join("_generated");
    if !schema_path.is_file() && !generated_dir.is_dir() {
        anyhow::bail!(
            "unsupported convex layout at {}: expected schema.ts or _generated/ metadata",
            layout.convex_root
        );
    }

    Ok(())
}

fn discover_convex_roots(root: &Utf8Path) -> Vec<Utf8PathBuf> {
    let mut candidates = vec![root.join("convex")];
    for base in ["apps", "packages"] {
        let dir = root.join(base);
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path().join("convex");
                if let Ok(path) = Utf8PathBuf::from_path_buf(path) {
                    candidates.push(path);
                }
            }
        }
    }
    candidates
        .into_iter()
        .filter(|path| path.is_dir())
        .collect()
}

fn discover_component_roots(convex_root: &Utf8Path) -> Vec<Utf8PathBuf> {
    let components_dir = convex_root.join("components");
    let Ok(entries) = std::fs::read_dir(&components_dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| Utf8PathBuf::from_path_buf(entry.path()).ok())
        .filter(|path| path.is_dir())
        .collect()
}
