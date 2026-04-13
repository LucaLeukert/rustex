use anyhow::Result;
use camino::Utf8Path;
use rustex_ir::IrPackage;
use rustex_rustgen::GeneratedFile;

pub fn write_ir(package: &IrPackage, out_dir: &Utf8Path) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;
    std::fs::write(
        out_dir.join("rustex.ir.json"),
        serde_json::to_string_pretty(package)?,
    )?;
    std::fs::write(
        out_dir.join("rustex.manifest.json"),
        serde_json::to_string_pretty(&package.manifest_meta)?,
    )?;
    std::fs::write(
        out_dir.join("rustex.diagnostics.json"),
        serde_json::to_string_pretty(&package.diagnostics)?,
    )?;
    Ok(())
}

pub fn write_rust(files: &[GeneratedFile], out_dir: &Utf8Path) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;
    for file in files {
        let path = out_dir.join(&file.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &file.contents)?;
    }
    Ok(())
}
