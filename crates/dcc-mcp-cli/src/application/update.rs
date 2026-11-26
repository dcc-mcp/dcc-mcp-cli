use std::io::Write;

use anyhow::Context;
use dcc_mcp_updater::Updater;
use reqwest::Url;
use semver::Version;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::domain::rest::Endpoint;

const CLI_MANIFEST_ENV: &str = "DCC_MCP_CLI_UPDATE_MANIFEST_URL";
const CLI_REPOSITORY: &str = "dcc-mcp/dcc-mcp-cli";
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_UPDATE_BYTES: u64 = 256 * 1024 * 1024;

enum UpdateSource {
    Gateway(Updater),
    Cli {
        binary_name: String,
        current_version: String,
        client: reqwest::Client,
    },
}

/// Service for updating the CLI itself or a gateway-managed runtime binary.
pub struct UpdateService {
    source: UpdateSource,
}

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
    sha256: String,
    release_notes: Option<String>,
    manifest_url: Url,
}

impl UpdateService {
    /// Create a service backed by the gateway update endpoint.
    pub fn new(gateway_url: &str, binary_name: &str, current_version: &str) -> Self {
        Self {
            source: UpdateSource::Gateway(Updater::new(gateway_url, binary_name, current_version)),
        }
    }

    /// Select the official CLI release manifest for CLI binaries and the
    /// gateway endpoint for server or other runtime binaries.
    pub fn for_binary(gateway_url: &str, binary_name: &str, current_version: &str) -> Self {
        if is_cli_binary(binary_name) {
            Self {
                source: UpdateSource::Cli {
                    binary_name: binary_name.to_owned(),
                    current_version: current_version.to_owned(),
                    client: reqwest::Client::builder()
                        .timeout(std::time::Duration::from_secs(30))
                        .build()
                        .expect("reqwest Client should build with default settings"),
                },
            }
        } else {
            Self::new(gateway_url, binary_name, current_version)
        }
    }

    pub fn with_endpoint(endpoint: &Endpoint, binary_name: &str, current_version: &str) -> Self {
        Self::for_binary(&endpoint.base_url, binary_name, current_version)
    }

    /// Check the selected update source without downloading an update.
    pub async fn check(&self) -> anyhow::Result<Value> {
        match &self.source {
            UpdateSource::Gateway(updater) => Ok(updater.check_update_json().await?.body),
            UpdateSource::Cli { .. } => Ok(self.cli_check_value().await?),
        }
    }

    /// Check, download, verify, and stage the selected binary for next launch.
    pub async fn apply(&self) -> anyhow::Result<Value> {
        match &self.source {
            UpdateSource::Gateway(updater) => self.apply_gateway_update(updater).await,
            UpdateSource::Cli { .. } => self.apply_cli_update().await,
        }
    }

    /// Best-effort self-update used by `--auto-update` and its environment
    /// variable equivalent. A staged update is reused without another network
    /// request.
    pub async fn auto_update(&self) -> anyhow::Result<Value> {
        let UpdateSource::Cli { binary_name, .. } = &self.source else {
            anyhow::bail!("automatic updates are only supported for the DCCMCP CLI");
        };
        if Updater::has_staged_update(binary_name)? {
            return Ok(serde_json::json!({
                "status": "pending",
                "binary_name": binary_name,
                "message": "A verified CLI update is already staged for the next launch."
            }));
        }
        self.apply_cli_update().await
    }

    async fn apply_gateway_update(&self, updater: &Updater) -> anyhow::Result<Value> {
        let info = updater.check_update().await?;
        if !info.update_available {
            return Ok(serde_json::json!({
                "status": "up-to-date",
                "current_version": info.current_version,
                "latest_version": info.latest_version,
                "message": "Already running the latest version."
            }));
        }

        let downloaded = updater.download_update(&info).await?;
        Updater::stage_update(&downloaded, updater.binary_name())?;

        Ok(serde_json::json!({
            "status": "staged",
            "current_version": info.current_version,
            "latest_version": info.latest_version,
            "staged_at": downloaded.to_string_lossy(),
            "message": "Update downloaded and staged. Restart the binary to apply."
        }))
    }

    async fn apply_cli_update(&self) -> anyhow::Result<Value> {
        let info = self.cli_update_info().await?;
        if info.latest_version <= parse_version(&info.current_version)? {
            return Ok(cli_update_value(
                &info,
                false,
                "Already running the latest version.",
            ));
        }

        let response = self
            .cli_client()
            .get(info.download_url.clone())
            .send()
            .await?
            .error_for_status()?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_UPDATE_BYTES)
        {
            anyhow::bail!("CLI update exceeds the {MAX_UPDATE_BYTES}-byte safety limit");
        }

        let bytes = response.bytes().await?;
        if bytes.is_empty() || bytes.len() as u64 > MAX_UPDATE_BYTES {
            anyhow::bail!("CLI update download is empty or exceeds the safety limit");
        }
        let actual = hex::encode(Sha256::digest(&bytes));
        if actual != info.sha256.as_str() {
            anyhow::bail!(
                "CLI update checksum mismatch: expected {}, got {}",
                info.sha256.as_str(),
                actual
            );
        }

        let mut temporary =
            tempfile::NamedTempFile::new().context("create CLI update staging file")?;
        temporary.write_all(&bytes)?;
        temporary.as_file().sync_all()?;
        Updater::stage_update(temporary.path(), &info.binary_name)?;

        Ok(cli_update_value(
            &info,
            true,
            "Update downloaded, verified, and staged. Restart the CLI to apply it.",
        ))
    }

    async fn cli_check_value(&self) -> anyhow::Result<Value> {
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
        let UpdateSource::Cli {
            binary_name,
            current_version,
            ..
        } = &self.source
        else {
            anyhow::bail!("the selected update source is not the CLI release manifest");
        };
        let manifest_url = cli_manifest_url()?;
        let response = self
            .cli_client()
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
        let bytes = response.bytes().await?;
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
        let sha256 = normalize_sha256(&entry.sha256)?;

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

    fn cli_client(&self) -> &reqwest::Client {
        match &self.source {
            UpdateSource::Cli { client, .. } => client,
            UpdateSource::Gateway(_) => unreachable!("gateway updates do not use the CLI client"),
        }
    }
}

fn is_cli_binary(name: &str) -> bool {
    matches!(name, "dccmcp" | "dcc-mcp-cli")
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
        "sha256": info.sha256,
        "release_notes": info.release_notes,
        "manifest_url": info.manifest_url,
        "source": "official-cli-release-manifest",
        "message": message,
    })
}

fn normalize_sha256(value: &str) -> anyhow::Result<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.len() != 64 || !normalized.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        anyhow::bail!("CLI update manifest contains an invalid SHA-256 digest");
    }
    Ok(normalized)
}
