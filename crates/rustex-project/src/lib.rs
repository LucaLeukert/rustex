use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

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
            allow_inferred_returns: false,
            naming_strategy: default_naming(),
            id_style: default_id_style(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectLayout {
    pub root: Utf8PathBuf,
    pub convex_root: Utf8PathBuf,
    pub out_dir: Utf8PathBuf,
    pub config_path: Utf8PathBuf,
}

pub fn load_config(root: &Utf8Path) -> Result<(RustexConfig, ProjectLayout)> {
    let config_path = root.join("rustex.toml");
    let raw = std::fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read config at {config_path}"))?;
    let config: RustexConfig =
        toml::from_str(&raw).with_context(|| format!("failed to parse {config_path}"))?;

    let layout = ProjectLayout {
        root: root.to_path_buf(),
        convex_root: absolutize(root, &config.convex_root),
        out_dir: absolutize(root, &config.out_dir),
        config_path,
    };

    Ok((config, layout))
}

fn absolutize(root: &Utf8Path, path: &Utf8Path) -> Utf8PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}
