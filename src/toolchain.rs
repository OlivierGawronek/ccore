use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Finds a C compiler: `CCORE_CC` if set, otherwise the first of clang, gcc or cc on PATH.
pub fn find_c_compiler() -> Option<String> {
    if let Ok(cc) = env::var("CCORE_CC") {
        return Some(cc);
    }
    ["clang", "gcc", "cc"].into_iter().find_map(|cc| {
        let ok = Command::new(cc)
            .arg("--version")
            .output()
            .is_ok_and(|out| out.status.success());
        ok.then(|| cc.to_string())
    })
}

pub fn compile_c(cc: &str, c_code: &str, output: &Path) -> Result<(), String> {
    let stem = output.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
    let c_path = env::temp_dir().join(format!("ccore-{}-{stem}.c", std::process::id()));
    fs::write(&c_path, c_code).map_err(|e| format!("cannot write {}: {e}", c_path.display()))?;

    let result = Command::new(cc)
        .args(["-std=c11", "-O2", "-o"])
        .arg(output)
        .arg(&c_path)
        .output();
    let _ = fs::remove_file(&c_path);

    let out = result.map_err(|e| format!("failed to run `{cc}`: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(())
}
