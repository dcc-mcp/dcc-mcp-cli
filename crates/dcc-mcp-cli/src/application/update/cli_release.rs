use std::io::Write;

use anyhow::Context;
use dcc_mcp_updater::{Sha256Digest, Updater};
use reqwest::Url;
use semver::Version;
use serde::Deserialize;
use serde_json::Value;

const CLI_MANIFEST_ENV: &str = "DCC_MCP_CLI_UPDATE_MANIFEST_URL";
const CLI_REPOSITORY: &str = "dcc-mcp/dcc-mcp-cli";
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_UPDATE_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
struct CliManifestEntry {
    version: String,
    url: String,
    sha256: String,
    #[serde(default)]
    release_notes: Option<String>,
}

#[derive(Debug)]
struct CliUpdateInfo {
    binary_name: String,
    current_version: String,
    latest_version: Version,
    latest_version_text: String,
    download_url: Url,
    sha256: Sha256Digest,
    release_notes: Option<String>,
    manifest_url: Url,
}

pub(super) struct CliRelease {
    binary_name: String,
    current_version: String,
    client: reqwest::Client,
}

impl CliRelease {
    pub(super) fn new(binary_name: &str, current_version: &str) -> Self {
        Self {
            binary_name: binary_name.into(),
            current_version: current_version.into(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("default TLS client"),
        }
    }
    pub(super) async fn apply(&self) -> anyhow::Result<Value> {
        let info = self.cli_update_info().await?;
        if info.latest_version <= parse_version(&info.current_version)? {
            return Ok(cli_update_value(
                &info,
                false,
                "Already running the latest version.",
            ));
        }

        let mut response = self
            .client
            .get(info.download_url.clone())
            .send()
            .await?
            .error_for_status()?;
        if response
            .content_length()
            .is_some_and(|size| size > MAX_UPDATE_BYTES)
        {
            anyhow::bail!("CLI update exceeds the {MAX_UPDATE_BYTES}-byte safety limit");
        }
        let mut temporary =
            tempfile::NamedTempFile::new().context("create CLI update staging file")?;
        let mut received = 0_u64;
        while let Some(chunk) = response.chunk().await? {
            received += chunk.len() as u64;
            if received > MAX_UPDATE_BYTES {
                anyhow::bail!("CLI update exceeds the {MAX_UPDATE_BYTES}-byte safety limit");
            }
            temporary.write_all(&chunk)?;
        }
        if received == 0 {
            anyhow::bail!("empty CLI update asset");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            // Staging copies this file's mode to the installed executable.
            // Preserve its access restrictions and omit privileged or shared-write bits.
            let installed = crate::application::current_exe::current_exe()?;
            let mode = std::fs::metadata(installed)?.permissions().mode() & 0o755;
            temporary
                .as_file()
                .set_permissions(std::fs::Permissions::from_mode(mode))?;
        }
        temporary.as_file().sync_all()?;
        Updater::stage_verified_update(temporary.path(), &info.binary_name, &info.sha256)?;

        Ok(cli_update_value(
            &info,
            true,
            "Update downloaded, verified, and staged. Restart the CLI to apply it.",
        ))
    }

    pub(super) async fn check(&self) -> anyhow::Result<Value> {
        let info = self.cli_update_info().await?;
        let current = parse_version(&info.current_version)?;
        Ok(cli_update_value(
            &info,
            info.latest_version > current,
            if info.latest_version > current {
                "A newer CLI version is available."
            } else {
                "Already running the latest version."
            },
        ))
    }

    async fn cli_update_info(&self) -> anyhow::Result<CliUpdateInfo> {
        let binary_name = &self.binary_name;
        let current_version = &self.current_version;
        let manifest_url = cli_manifest_url()?;
        let mut response = self
            .client
            .get(manifest_url.clone())
            .send()
            .await?
            .error_for_status()?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_MANIFEST_BYTES)
        {
            anyhow::bail!("CLI update manifest exceeds the {MAX_MANIFEST_BYTES}-byte safety limit");
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if bytes.len() + chunk.len() > MAX_MANIFEST_BYTES as usize {
                anyhow::bail!("CLI update manifest exceeds the safety limit");
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.is_empty() || bytes.len() as u64 > MAX_MANIFEST_BYTES {
            anyhow::bail!("CLI update manifest is empty or exceeds the safety limit");
        }
        let manifest: Value =
            serde_json::from_slice(&bytes).context("decode CLI update manifest")?;
        let entry = manifest
            .get(binary_name)
            .or_else(|| manifest.get("dccmcp"))
            .or_else(|| manifest.get("dcc-mcp-cli"))
            .cloned()
            .context("CLI update manifest has no entry for dccmcp")?;
        let entry: CliManifestEntry =
            serde_json::from_value(entry).context("decode CLI update manifest entry")?;
        let latest_version = parse_version(&entry.version)?;
        let download_url = Url::parse(&entry.url).context("invalid CLI update download URL")?;
        validate_download_url(&manifest_url, &download_url)?;
        let sha256 = Sha256Digest::require(binary_name, Some(entry.sha256.trim()))?;

        Ok(CliUpdateInfo {
            binary_name: binary_name.clone(),
            current_version: current_version.clone(),
            latest_version,
            latest_version_text: entry.version,
            download_url,
            sha256,
            release_notes: entry.release_notes,
            manifest_url,
        })
    }
}

fn cli_manifest_url() -> anyhow::Result<Url> {
    let value = std::env::var(CLI_MANIFEST_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "https://github.com/{CLI_REPOSITORY}/releases/latest/download/dcc-mcp-update-manifest-{}.json",
                release_platform()
            )
        });
    let url = Url::parse(&value).context("invalid CLI update manifest URL")?;
    if url.scheme() != "https" && !is_loopback_url(&url) {
        anyhow::bail!("CLI update manifest must use HTTPS, except for a loopback test endpoint");
    }
    Ok(url)
}

fn validate_download_url(manifest_url: &Url, download_url: &Url) -> anyhow::Result<()> {
    if download_url.scheme() != "https" && !is_loopback_url(download_url) {
        anyhow::bail!("CLI update download must use HTTPS, except for a loopback test endpoint");
    }
    let manifest_host = manifest_url.host_str();
    let download_host = download_url.host_str();
    if manifest_host != download_host
        && !matches!(
            download_host,
            Some("github.com" | "objects.githubusercontent.com")
        )
    {
        anyhow::bail!("CLI update download host does not match the release manifest");
    }
    Ok(())
}

fn is_loopback_url(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
        && url
            .host_str()
            .is_some_and(|host| matches!(host, "127.0.0.1" | "localhost" | "::1"))
}

fn release_platform() -> &'static str {
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "windows-x86_64"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "linux-x86_64"
    } else if cfg!(all(
        target_os = "macos",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )) {
        "macos-universal2"
    } else {
        "unsupported"
    }
}

fn parse_version(value: &str) -> anyhow::Result<Version> {
    Version::parse(value.trim_start_matches('v'))
        .with_context(|| format!("invalid semantic version: {value}"))
}

fn cli_update_value(info: &CliUpdateInfo, update_available: bool, message: &str) -> Value {
    serde_json::json!({
        "status": if update_available { "update-available" } else { "up-to-date" },
        "update_available": update_available,
        "binary_name": info.binary_name,
        "current_version": info.current_version,
        "latest_version": info.latest_version_text,
        "download_url": info.download_url,
        "sha256": info.sha256.as_str(),
        "release_notes": info.release_notes,
        "manifest_url": info.manifest_url,
        "source": "official-cli-release-manifest",
        "message": message,
    })
}
