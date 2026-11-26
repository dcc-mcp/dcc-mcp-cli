use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;

const CUA_BINARY_ENV: &str = "DCC_MCP_CUA_BINARY";
const CUA_BINARY_NAME: &str = "dcc-cua";
const CUA_INSTALL_DIR_ENV: &str = "DCC_MCP_INSTALL_DIR";

/// Resolve and execute the independently released dcc-cua runtime.
///
/// The CLI intentionally owns no CUA SDK or host lifecycle code. It only
/// resolves the companion executable, forwards its arguments, and preserves
/// the child's standard streams and exit status.
pub fn run(args: &[OsString]) -> anyhow::Result<i32> {
    let executable = resolve_binary()?;
    let status = Command::new(&executable)
        .args(args)
        .status()
        .with_context(|| format!("failed to start {}", executable.display()))?;
    Ok(status.code().unwrap_or(1))
}

fn resolve_binary() -> anyhow::Result<PathBuf> {
    let configured = std::env::var_os(CUA_BINARY_ENV);
    let current_exe = std::env::current_exe().ok();
    let install_dir = std::env::var_os(CUA_INSTALL_DIR_ENV);
    let path = std::env::var_os("PATH");
    resolve_binary_from(
        configured.as_deref(),
        current_exe.as_deref(),
        install_dir.as_deref(),
        path.as_deref(),
    )
}

fn resolve_binary_from(
    configured: Option<&OsStr>,
    current_exe: Option<&Path>,
    install_dir: Option<&OsStr>,
    path: Option<&OsStr>,
) -> anyhow::Result<PathBuf> {
    if let Some(value) = configured.filter(|value| !value.is_empty()) {
        let candidate = PathBuf::from(value);
        if !candidate.is_absolute() {
            anyhow::bail!("{CUA_BINARY_ENV} must be an absolute path");
        }
        if !candidate.is_file() {
            anyhow::bail!(
                "{CUA_BINARY_ENV} does not name a file: {}",
                candidate.display()
            );
        }
        return Ok(candidate);
    }

    if let Some(current_exe) = current_exe
        && let Some(directory) = current_exe.parent()
        && let Some(candidate) = existing_candidate(directory)
    {
        return Ok(candidate);
    }

    for directory in standard_install_directories(install_dir) {
        if let Some(candidate) = existing_candidate(&directory) {
            return Ok(candidate);
        }
    }

    if let Some(candidate) = versioned_installation() {
        return Ok(candidate);
    }

    if let Some(path) = path {
        for directory in std::env::split_paths(path) {
            if let Some(candidate) = existing_candidate(&directory) {
                return Ok(candidate);
            }
        }
    }

    anyhow::bail!(concat!(
        "dcc-cua was not found. Install the independently released dcc-cua runtime, or set ",
        "DCC_MCP_CUA_BINARY to its absolute path."
    ))
}

fn existing_candidate(directory: &Path) -> Option<PathBuf> {
    let names = if cfg!(windows) {
        ["dcc-cua.exe", CUA_BINARY_NAME]
    } else {
        [CUA_BINARY_NAME, "dcc-cua.exe"]
    };
    names
        .into_iter()
        .map(|name| directory.join(name))
        .find(|candidate| candidate.is_file())
}

fn standard_install_directories(configured: Option<&OsStr>) -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(value) = configured.filter(|value| !value.is_empty()) {
        let directory = PathBuf::from(value);
        if directory.is_absolute() {
            directories.push(directory);
        }
    }

    if cfg!(windows) {
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            directories.push(PathBuf::from(local_app_data).join("dcc-mcp").join("bin"));
        }
    } else if let Some(home) = std::env::var_os("HOME") {
        directories.push(PathBuf::from(home).join(".local").join("bin"));
    }
    directories
}

fn versioned_installation() -> Option<PathBuf> {
    let root = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("dcc-cua").join("versions"))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(PathBuf::from).map(|path| {
            path.join("Library")
                .join("Application Support")
                .join("dcc-cua")
                .join("versions")
        })
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|path| path.join(".local").join("share"))
            })
            .map(|path| path.join("dcc-cua").join("versions"))
    }?;

    let mut candidates = std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let version = entry
                .file_name()
                .to_str()?
                .parse::<semver::Version>()
                .ok()?;
            let executable = existing_candidate(&path)?;
            Some((version, executable))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.0.cmp(&right.0));
    candidates.pop().map(|(_, path)| path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn explicit_binary_must_be_absolute_and_existing() {
        let root = tempdir().unwrap();
        let binary = root.path().join("dcc-cua");
        std::fs::write(&binary, b"test").unwrap();

        let resolved = resolve_binary_from(Some(binary.as_os_str()), None, None, None).unwrap();
        assert_eq!(resolved, binary);
    }

    #[test]
    fn cli_sibling_is_preferred_to_path() {
        let root = tempdir().unwrap();
        let sibling = root.path().join(if cfg!(windows) {
            "dcc-cua.exe"
        } else {
            "dcc-cua"
        });
        let path_dir = root.path().join("path");
        let path_binary = path_dir.join(if cfg!(windows) {
            "dcc-cua.exe"
        } else {
            "dcc-cua"
        });
        std::fs::create_dir(&path_dir).unwrap();
        std::fs::write(&sibling, b"sibling").unwrap();
        std::fs::write(&path_binary, b"path").unwrap();

        let current_exe = root.path().join(if cfg!(windows) {
            "dccmcp.exe"
        } else {
            "dccmcp"
        });
        let resolved =
            resolve_binary_from(None, Some(&current_exe), None, Some(path_dir.as_os_str()))
                .unwrap();
        assert_eq!(resolved, sibling);
    }

    #[test]
    fn invalid_configured_binary_reports_resolution_error() {
        let root = tempdir().unwrap();
        let missing = root.path().join("missing-dcc-cua");
        let error = resolve_binary_from(Some(missing.as_os_str()), None, None, None).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("DCC_MCP_CUA_BINARY does not name a file"));
    }
}
