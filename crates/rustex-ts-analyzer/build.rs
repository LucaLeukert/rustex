use std::{
    env,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    if let Err(error) = build() {
        panic!("failed to build TypeScript analyzer: {error}");
    }
}

fn build() -> Result<(), String> {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").map_err(|err| err.to_string())?);
    let package_dir = manifest_dir.join("../../packages/ts-analyzer");
    let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|err| err.to_string())?);
    let analyzer_out_dir = out_dir.join("ts-analyzer");

    println!(
        "cargo:rerun-if-changed={}",
        package_dir.join("src/analyze.ts").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        package_dir.join("package.json").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        package_dir.join("pnpm-lock.yaml").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        package_dir.join("tsconfig.build.json").display()
    );

    let _node = find_command("RUSTEX_NODE_BIN", &["node", "nodejs"])?;
    ensure_node_modules(&package_dir)?;
    let esbuild = esbuild_path(&package_dir);
    if !esbuild.is_file() {
        return Err(format!(
            "esbuild executable not found at {} after dependency install",
            esbuild.display()
        ));
    }

    std::fs::create_dir_all(&analyzer_out_dir)
        .map_err(|err| format!("failed to create analyzer output directory: {err}"))?;
    let entrypoint = analyzer_out_dir.join("analyze.cjs");

    run_command(
        &esbuild,
        &[
            package_dir.join("src/analyze.ts").as_os_str(),
            OsStr::new("--bundle"),
            OsStr::new("--minify"),
            OsStr::new("--platform=node"),
            OsStr::new("--format=cjs"),
            OsStr::new("--target=node20"),
            OsStr::new(&format!("--outfile={}", entrypoint.display())),
        ],
        Some(&package_dir),
        "failed to bundle TypeScript analyzer",
    )?;

    if !entrypoint.is_file() {
        return Err(format!(
            "expected bundled analyzer entrypoint at {}",
            entrypoint.display()
        ));
    }
    let bundle =
        fs::read(&entrypoint).map_err(|err| format!("failed to read bundled analyzer: {err}"))?;
    let bundle_hash = sha256_hex(&bundle);

    println!(
        "cargo:rustc-env=RUSTEX_TS_ANALYZER_BUNDLE={}",
        entrypoint.display()
    );
    println!("cargo:rustc-env=RUSTEX_TS_ANALYZER_BUNDLE_SHA256={bundle_hash}");
    Ok(())
}

fn ensure_node_modules(package_dir: &Path) -> Result<(), String> {
    let required = [
        package_dir.join("node_modules/typescript/lib/tsc.js"),
        esbuild_path(package_dir),
        package_dir.join("node_modules/effect/package.json"),
        package_dir.join("node_modules/@effect/cli/package.json"),
        package_dir.join("node_modules/@effect/printer/package.json"),
        package_dir.join("node_modules/@effect/printer-ansi/package.json"),
    ];
    if required.iter().all(|path| path.is_file()) {
        return Ok(());
    }

    let pnpm = find_command("RUSTEX_PNPM_BIN", &["pnpm", "pnpm.cmd"])?;
    run_command(
        &pnpm,
        &[
            OsStr::new("install"),
            OsStr::new("--frozen-lockfile"),
            OsStr::new("--ignore-workspace"),
        ],
        Some(package_dir),
        "failed to install ts-analyzer pnpm dependencies",
    )?;

    if required.iter().all(|path| path.is_file()) {
        Ok(())
    } else {
        Err(format!(
            "pnpm install completed but ts-analyzer dependencies are still missing under {}",
            package_dir.join("node_modules").display()
        ))
    }
}

fn esbuild_path(package_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        package_dir.join("node_modules/.bin/esbuild.cmd")
    } else {
        package_dir.join("node_modules/.bin/esbuild")
    }
}

fn find_command(env_var: &str, candidates: &[&str]) -> Result<PathBuf, String> {
    if let Ok(explicit) = env::var(env_var) {
        let path = PathBuf::from(explicit);
        return verify_command(&path)
            .map(|_| path)
            .map_err(|err| format!("{env_var} points to an unusable executable: {err}"));
    }

    for candidate in candidates {
        let path = PathBuf::from(candidate);
        if verify_command(&path).is_ok() {
            return Ok(path);
        }
    }

    Err(format!(
        "could not find a usable command for {} (tried: {})",
        env_var,
        candidates.join(", ")
    ))
}

fn verify_command(command: &Path) -> Result<(), String> {
    let output = Command::new(command)
        .arg("--version")
        .output()
        .map_err(|err| err.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("command exited with status {}", output.status))
    }
}

fn run_command(
    program: &Path,
    args: &[&OsStr],
    cwd: Option<&Path>,
    context: &str,
) -> Result<(), String> {
    let mut command = Command::new(program);
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = command
        .output()
        .map_err(|err| format!("{context}: {err}"))?;
    if output.status.success() {
        return Ok(());
    }

    Err(format!(
        "{context}: status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    ))
}

fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let digest = {
        use sha2::Digest as _;
        sha2::Sha256::digest(bytes)
    };
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}
