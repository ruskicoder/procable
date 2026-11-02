//! String-level guards over the two launchers (`skill/scripts/procable`,
//! `skill/scripts/procable.cmd`). The .cmd cannot be executed here (no Windows),
//! so this pins the shapes a dry parse depends on: asset naming in both
//! launchers agrees with `engine_binary::asset_url`, the .cmd contains no
//! multi-line parenthesized blocks (the parse-time `%var%` expansion bug), both
//! launchers carry the engine-probe handshake and verify cached engines against
//! the manifest, and (procable) neither launcher can download anything.

use procable_skills::engine_binary::{asset_url, DEFAULT_DOWNLOAD_BASE};

/// The launchers ship next to the skill they power.
fn launcher_dir() -> String {
    format!("{}/../../skill/scripts", env!("CARGO_MANIFEST_DIR"))
}

fn launcher_file(name: &str) -> String {
    let path = format!("{}/{name}", launcher_dir());
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

#[test]
fn sh_launcher_asset_naming_matches_engine() {
    let sh = launcher_file("procable");
    // The cache and manifest key is procable-<os>-<arch>[.exe], as asset_url names it.
    assert!(sh.contains(r#"asset="procable-$os-$arch""#));
    let win_x64 = asset_url(DEFAULT_DOWNLOAD_BASE, "V", "windows", "x64");
    assert!(win_x64.ends_with("/engine-vV/procable-windows-x64.exe"));
    // The PATH and unversioned home candidates are probed.
    assert!(sh.contains("engine-probe"));
    assert!(sh.contains(r#"probe_ok "$home_bin""#));
    assert!(sh.contains("probe_ok procable"));
    assert!(!sh.contains("npm i -g"));
    assert!(!sh.contains("npm install"));
}

#[test]
fn cmd_launcher_asset_naming_matches_engine() {
    let cmd = launcher_file("procable.cmd");
    assert!(cmd.contains(r#"set "asset=procable-windows-%arch%.exe""#));
    assert!(cmd.contains(r#"if /I "%PROCESSOR_ARCHITECTURE%"=="ARM64" set "arch=arm64""#));
    // sha256 verification of a cached engine via certutil against the committed manifest.
    assert!(cmd.contains("certutil -hashfile"));
    assert!(cmd.contains("engine-checksums.txt"));
    // engine-probe handshake for PATH / unversioned home candidates.
    assert!(cmd.contains("engine-probe"));
    assert!(cmd.contains(r#"findstr /b /c:"procable-engine""#));
    assert!(!cmd.contains("npm i -g"));
}

#[test]
fn cmd_launcher_forwards_engine_exit_code() {
    // Bare `exit /b` drops the process exit code when this file is cmd.exe's
    // entry point (cmd /c, PowerShell, Node spawn). Forward %errorlevel%
    // after each engine invocation instead.
    let cmd = launcher_file("procable.cmd");
    for (i, line) in cmd.lines().enumerate() {
        assert_ne!(
            line.trim(),
            "exit /b",
            "procable.cmd line {}: bare exit /b drops the process code: {line}",
            i + 1
        );
    }
    let forward = "exit /b %errorlevel%";
    assert_eq!(
        cmd.matches(forward).count(),
        2,
        "PATH candidate and :run must both forward the engine exit code"
    );
}

#[test]
fn cmd_launcher_has_no_multiline_parenthesized_blocks() {
    // cmd.exe expands %var% inside a parenthesized block at parse time, so a
    // `set` + read-back inside one block silently reads the pre-block value
    // (rollout review S3: the download path could never fire). The launcher
    // is written as straight-line goto flow; keep it that way.
    let cmd = launcher_file("procable.cmd");
    for (i, line) in cmd.lines().enumerate() {
        assert!(
            !line.trim_end().ends_with('('),
            "procable.cmd line {}: opens a multi-line parenthesized block: {line}",
            i + 1
        );
    }
}

#[test]
fn launchers_verify_cached_engines_against_the_manifest() {
    // A cached engine runs only while its stamp or hash matches the digest
    // committed in engine-checksums.txt; anything else is refused.
    let sh = launcher_file("procable");
    let cmd = launcher_file("procable.cmd");
    assert!(sh.contains(r#"manifest="$dir/engine-checksums.txt""#));
    assert!(cmd.contains(r#"set "manifest=%~dp0engine-checksums.txt""#));
    for text in [&sh, &cmd] {
        assert!(!text.contains(".sha256"));
        assert!(text.contains(".verified"));
        assert!(text.contains("cannot verify the cached engine"));
    }
}

#[test]
fn launchers_never_download() {
    // procable runs offline: no fetch tool, no release URL, no download variable.
    let sh = launcher_file("procable");
    let cmd = launcher_file("procable.cmd");
    for text in [&sh, &cmd] {
        for banned in ["curl", "wget", "PROCABLE_DOWNLOAD_BASE", "releases/download", "https://"] {
            assert!(!text.contains(banned), "launcher still contains {banned}");
        }
        assert!(text.contains("runs offline and does not download its engine"));
    }
    assert_eq!(
        asset_url(DEFAULT_DOWNLOAD_BASE, "1.2.3", "darwin", "arm64"),
        format!("{DEFAULT_DOWNLOAD_BASE}/engine-v1.2.3/procable-darwin-arm64")
    );
}

#[test]
fn sh_launcher_exports_skill_dir_before_the_env_bin_exec() {
    // Regression: the launcher used to `exec "$PROCABLE_BIN"` BEFORE it
    // exported PROCABLE_SKILL_DIR/SELF, so a binary reached via PROCABLE_BIN
    // ran with no skill dir and could not inline reference/*.md or read its own
    // version (native platform refs and UPDATE_AVAILABLE silently vanished).
    // The skill-behavior suite caught it; the oracle could not (it sets the env
    // directly). Guard the ordering at the string level and functionally.
    let sh = launcher_file("procable");
    let export_pos = sh
        .find("export PROCABLE_SKILL_DIR PROCABLE_SELF")
        .expect("launcher exports the skill-dir env");
    let env_bin_exec = sh
        .find(r#"exec "${PROCABLE_BIN}""#)
        .expect("launcher execs PROCABLE_BIN");
    assert!(
        export_pos < env_bin_exec,
        "PROCABLE_SKILL_DIR must be exported before the PROCABLE_BIN exec"
    );
}

#[cfg(unix)]
#[test]
fn sh_launcher_passes_skill_dir_to_the_env_bin() {
    use std::os::unix::fs::PermissionsExt;
    let launcher = format!("{}/procable", launcher_dir());
    let dir = std::env::temp_dir().join(format!("procable-launcher-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // A stub "engine binary" that just prints the skill dir it was handed.
    let stub = dir.join("stub");
    std::fs::write(&stub, "#!/bin/sh\nprintf 'SKILL_DIR=%s\\n' \"${PROCABLE_SKILL_DIR:-UNSET}\"\n").unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    let out = std::process::Command::new("sh")
        .arg(&launcher)
        .arg("context")
        .env("PROCABLE_BIN", &stub)
        .env_remove("PROCABLE_SKILL_DIR")
        .output()
        .expect("run launcher");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("SKILL_DIR=") && !stdout.contains("SKILL_DIR=UNSET") && !stdout.contains("SKILL_DIR=\n"),
        "launcher must export a non-empty PROCABLE_SKILL_DIR to the PROCABLE_BIN binary; got: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
