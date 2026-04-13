use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn generates_outputs_for_basic_fixture() -> Result<()> {
    let fixture = workspace_root().join("fixtures/basic-schema");
    let temp = std::env::temp_dir().join(format!(
        "rustex-fixture-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ));
    copy_dir(&fixture, &temp)?;

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

    Ok(())
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
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
