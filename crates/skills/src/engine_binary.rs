//! Engine binary paths, asset names and checksum manifest parsing, plus the
//! post-install check that a skill directory carries its engine. procable runs
//! offline: nothing here downloads. The asset naming and the checksum manifest
//! stay because the release workflow and the launcher contract tests use them.

use impeccable_common::Io;

use crate::providers::Sys;
use crate::util::{self, jsp};

/// Where the fork publishes engine binaries; used to name assets, never fetched at runtime.
pub const DEFAULT_DOWNLOAD_BASE: &str = "https://github.com/ruskicoder/procable/releases/download";

/// The `<os>-<arch>` tag the launcher computes (`darwin|linux|windows` x
/// `arm64|x64`); `None` on a platform without a release asset.
pub fn platform_tag() -> Option<(&'static str, &'static str)> {
    let os = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        return None;
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86_64") {
        "x64"
    } else {
        return None;
    };
    Some((os, arch))
}

/// Where the binary for `(os, arch)` lives inside a skill directory.
pub fn binary_path(skill_dir: &str, os: &str, arch: &str) -> String {
    let name = if os == "windows" { "impeccable.exe" } else { "impeccable" };
    jsp::join(&[skill_dir, "scripts", "bin", &format!("{os}-{arch}"), name])
}

/// The release asset URL for a version and platform. Engine releases are
/// tagged `engine-v<version>` because the repo also carries skill, CLI,
/// extension and detector tags.
pub fn asset_url(base: &str, version: &str, os: &str, arch: &str) -> String {
    format!("{}/engine-v{version}/{}", base.trim_end_matches('/'), asset_name(os, arch))
}

fn asset_name(os: &str, arch: &str) -> String {
    let ext = if os == "windows" { ".exe" } else { "" };
    format!("impeccable-{os}-{arch}{ext}")
}

/// The digest `engine-checksums.txt` records for `version` and `asset`. Lines
/// are `<version> <asset> <sha256>`; blank lines and `#` comments are skipped.
fn manifest_digest(manifest: &str, version: &str, asset: &str) -> Option<String> {
    manifest.lines().find_map(|line| {
        let mut f = line.split_whitespace();
        match (f.next(), f.next(), f.next()) {
            (Some(v), Some(a), Some(sum)) if v == version && a == asset => Some(sum.to_lowercase()),
            _ => None,
        }
    })
}

/// Belt and suspenders for the executable bit: chmod +x the launcher
/// (`scripts/impeccable`) and every staged engine binary
/// (`scripts/bin/**/impeccable*`) inside a skill dir. Zip extraction and the
/// dir copy preserve modes themselves, but any channel that loses them (a
/// zip with no unix modes, a mode-dropping copier) would otherwise turn every
/// hook and Setup call into "Permission denied". No-op off unix and for
/// missing paths.
pub fn ensure_executable_scripts(skill_dir: &str) {
    let launcher = jsp::join(&[skill_dir, "scripts", "impeccable"]);
    if util::exists(&launcher) {
        let _ = util::set_executable(&launcher);
    }
    let bin_root = jsp::join(&[skill_dir, "scripts", "bin"]);
    for tag in util::read_dir_names(&bin_root).unwrap_or_default() {
        let dir = jsp::join(&[&bin_root, &tag]);
        if !util::is_dir(&dir) {
            continue;
        }
        for name in util::read_dir_names(&dir).unwrap_or_default() {
            if name.starts_with("impeccable") {
                let _ = util::set_executable(&jsp::join(&[&dir, &name]));
            }
        }
    }
}

/// Repair the executable bit on the launcher and the engine binaries in every
/// skill dir in `skill_dirs`, and report a skill dir with no engine for this
/// platform. procable runs offline: the engine is never downloaded. The
/// prompt-orchestrator installer ships it next to the launcher.
pub fn install_engine_binaries(sys: &Sys, io: &mut Io, skill_dirs: &[String]) {
    for skill_dir in skill_dirs {
        ensure_executable_scripts(skill_dir);
    }
    let Some((os, arch)) = platform_tag() else { return };
    for skill_dir in skill_dirs {
        let dest = binary_path(skill_dir, os, arch);
        if !util::exists(&dest) {
            io.err(&format!(
                "No procable engine for {os}-{arch} at {}. procable does not download it; reinstall with the prompt-orchestrator installer.\n",
                sys.format_path_for_display(&dest)
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_engine_is_reported_and_never_downloaded() {
        let Some((os, arch)) = platform_tag() else { return };
        let dir = std::env::temp_dir().join(format!("procable-no-engine-{}", std::process::id()));
        let skill_dir = dir.to_string_lossy().into_owned();
        std::fs::create_dir_all(dir.join("scripts")).unwrap();
        std::fs::write(dir.join("scripts/VERSION"), "9.9.9\n").unwrap();
        let env = util::Env::new();
        let (mut io, captured) = Io::captured("", dir.clone(), env.clone());
        install_engine_binaries(&Sys::new(env, skill_dir.clone()), &mut io, &[skill_dir.clone()]);
        let stderr = String::from_utf8_lossy(&captured.stderr.borrow()).into_owned();
        assert!(stderr.contains("does not download it"), "{stderr}");
        assert!(!util::exists(&binary_path(&skill_dir, os, arch)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn asset_naming_matches_launcher() {
        assert_eq!(
            asset_url(DEFAULT_DOWNLOAD_BASE, "1.2.3", "darwin", "arm64"),
            "https://github.com/ruskicoder/procable/releases/download/engine-v1.2.3/impeccable-darwin-arm64"
        );
        assert_eq!(asset_url("http://x/", "1", "windows", "x64"), "http://x/engine-v1/impeccable-windows-x64.exe");
        // The sibling binary path is joined with the host's path semantics
        // (backslashes on Windows); only the asset name is platform-keyed.
        assert_eq!(
            binary_path("/s", "linux", "x64"),
            jsp::join(&["/s", "scripts", "bin", "linux-x64", "impeccable"])
        );
        assert_eq!(
            binary_path("/s", "windows", "arm64"),
            jsp::join(&["/s", "scripts", "bin", "windows-arm64", "impeccable.exe"])
        );
    }

    #[test]
    fn manifest_digest_matches_version_and_asset() {
        let manifest = "# header: 1.2.3 impeccable-linux-x64 ffff\n\n\
            1.2.3 impeccable-linux-x64 ABC123\n\
            1.2.3 impeccable-windows-x64.exe def456\n\
            1.2.2 impeccable-linux-x64 old\n";
        assert_eq!(manifest_digest(manifest, "1.2.3", "impeccable-linux-x64").as_deref(), Some("abc123"));
        assert_eq!(manifest_digest(manifest, "1.2.3", "impeccable-windows-x64.exe").as_deref(), Some("def456"));
        assert_eq!(manifest_digest(manifest, "1.2.2", "impeccable-linux-x64").as_deref(), Some("old"));
        assert_eq!(manifest_digest(manifest, "1.2.3", "impeccable-darwin-arm64"), None);
        assert_eq!(manifest_digest(manifest, "9.9.9", "impeccable-linux-x64"), None);
        assert_eq!(manifest_digest("# only 1.2.3 impeccable-linux-x64 ffff\n", "1.2.3", "impeccable-linux-x64"), None);
        assert_eq!(manifest_digest("", "1.2.3", "impeccable-linux-x64"), None);
    }
}
