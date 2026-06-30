//! On-demand download of the cost-savings router model bundle.
//!
//! The router needs a model bundle in `~/.goose/complexity_model/` to do
//! anything. The bundle is large (~250MB) and not shipped with goose, so when
//! cost savings mode is enabled and no bundle is present we fetch it on first
//! use and install it atomically. Two sources are supported:
//!
//! - **A Hugging Face repo** (`GOOSE_ROUTER_BUNDLE_HF_REPO`): the bundle's files
//!   are downloaded individually, reusing goose's existing HF token resolution
//!   so private repos work with `HF_TOKEN` / the HF OAuth login.
//! - **A zip archive URL** (`GOOSE_ROUTER_BUNDLE_URL`, optionally verified with
//!   `GOOSE_ROUTER_BUNDLE_SHA256`).
//!
//! All failures are non-fatal: the router fails open (cost savings mode simply
//! does nothing) so a download problem never blocks a session.

use std::io::Read;
use std::path::Path;

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::providers::huggingface_auth;

const BUNDLE_HF_REPO_KEY: &str = "GOOSE_ROUTER_BUNDLE_HF_REPO";
const BUNDLE_HF_REVISION_KEY: &str = "GOOSE_ROUTER_BUNDLE_HF_REVISION";
const BUNDLE_HF_PATH_KEY: &str = "GOOSE_ROUTER_BUNDLE_HF_PATH";
const BUNDLE_URL_KEY: &str = "GOOSE_ROUTER_BUNDLE_URL";
const BUNDLE_SHA256_KEY: &str = "GOOSE_ROUTER_BUNDLE_SHA256";
const COST_SAVINGS_MODE_KEY: &str = "GOOSE_COST_SAVINGS_MODE";

const HF_API_BASE: &str = "https://huggingface.co/api/models";
const HF_DOWNLOAD_BASE: &str = "https://huggingface.co";

/// The Hugging Face repo the bundle is downloaded from when none is configured.
const DEFAULT_HF_REPO: &str = "micdn/llm-router-goose-public";

/// Where the bundle lives inside an HF repo, when not overridden. Published
/// router repos keep the bundle under this subdirectory.
const DEFAULT_HF_BUNDLE_PATH: &str = "embedding/complexity_model";

/// The bundle files the loader needs (relative to the repo subdirectory). Used
/// to validate that a repo/path actually contains a bundle before installing.
const REQUIRED_BUNDLE_FILES: &[&str] = &[
    "config.json",
    "embedder.onnx",
    "tokenizer.json",
    "weights.safetensors",
];

/// Ensure the router bundle is present when cost savings mode is enabled,
/// downloading it on first use. Does nothing (and never fails the caller) when
/// the mode is off, a bundle already exists, or no download source is set.
pub async fn ensure_bundle_if_enabled() {
    let config = Config::global();
    let enabled = config
        .get_param::<bool>(COST_SAVINGS_MODE_KEY)
        .unwrap_or(false);
    if !enabled {
        return;
    }
    if goose_router::bundle_present() {
        return;
    }

    let Some(dir) = goose_router::default_bundle_dir() else {
        tracing::warn!(
            target: "goose::router",
            "could not resolve the router bundle directory; routing is disabled",
        );
        return;
    };

    let hf_repo = config
        .get_param::<String>(BUNDLE_HF_REPO_KEY)
        .ok()
        .filter(|s| !s.trim().is_empty());
    let zip_url = config
        .get_param::<String>(BUNDLE_URL_KEY)
        .ok()
        .filter(|s| !s.trim().is_empty());

    // A zip URL is only used when explicitly set and no HF repo is configured;
    // otherwise the bundle comes from a Hugging Face repo (the built-in default
    // when none is set), so the feature works out of the box.
    let result = if let (None, Some(url)) = (&hf_repo, &zip_url) {
        let expected_sha256 = config
            .get_param::<String>(BUNDLE_SHA256_KEY)
            .ok()
            .filter(|s| !s.trim().is_empty());
        tracing::info!(
            target: "goose::router",
            url = %url,
            path = %dir.display(),
            "downloading cost-savings router bundle",
        );
        download_zip_and_install(url, expected_sha256.as_deref(), &dir).await
    } else {
        let repo = hf_repo.unwrap_or_else(|| DEFAULT_HF_REPO.to_string());
        let revision = config
            .get_param::<String>(BUNDLE_HF_REVISION_KEY)
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "main".to_string());
        let path_prefix = config
            .get_param::<String>(BUNDLE_HF_PATH_KEY)
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_HF_BUNDLE_PATH.to_string());
        tracing::info!(
            target: "goose::router",
            repo = %repo,
            revision = %revision,
            path_prefix = %path_prefix,
            path = %dir.display(),
            "downloading cost-savings router bundle from Hugging Face",
        );
        download_hf_repo_and_install(&repo, &revision, &path_prefix, &dir).await
    };

    match result {
        Ok(()) => {
            tracing::info!(
                target: "goose::router",
                path = %dir.display(),
                "router bundle installed",
            );
        }
        Err(e) => {
            tracing::warn!(
                target: "goose::router",
                error = %format!("{e:#}"),
                "failed to download router bundle; routing is disabled",
            );
        }
    }
}

/// List a Hugging Face repo's files, download every bundle file found under
/// `path_prefix` into a staging directory (stripping the prefix so files land
/// at the bundle root), and swap it into place atomically. Reuses goose's HF
/// token resolution so private repos work transparently.
async fn download_hf_repo_and_install(
    repo: &str,
    revision: &str,
    path_prefix: &str,
    dir: &Path,
) -> Result<()> {
    let client = reqwest::Client::new();
    let token = huggingface_auth::resolve_token_async().await.ok().flatten();
    let auth = token
        .as_deref()
        .filter(|t| !t.is_empty())
        .map(|t| format!("Bearer {t}"));

    let filenames = list_hf_repo_files(&client, repo, revision, auth.as_deref()).await?;

    let prefix = path_prefix.trim_matches('/');
    // Map each repo file under the prefix to its bundle-relative path.
    let to_download: Vec<(String, String)> = filenames
        .into_iter()
        .filter_map(|f| {
            let rel = if prefix.is_empty() {
                Some(f.as_str())
            } else {
                f.strip_prefix(prefix).and_then(|r| r.strip_prefix('/'))
            };
            rel.filter(|r| !r.is_empty() && !r.contains('/'))
                .map(|r| (f.clone(), r.to_string()))
        })
        .collect();

    for required in REQUIRED_BUNDLE_FILES {
        if !to_download.iter().any(|(_, rel)| rel == required) {
            bail!("Hugging Face repo {repo} (path {prefix:?}) is missing bundle file {required}");
        }
    }

    let staging = new_staging_dir(dir)?;
    for (repo_path, rel) in &to_download {
        let url = format!("{HF_DOWNLOAD_BASE}/{repo}/resolve/{revision}/{repo_path}");
        let mut request = client.get(&url).header("User-Agent", "goose-ai-agent");
        if let Some(header) = auth.as_deref() {
            request = request.header("Authorization", header);
        }
        let bytes = request
            .send()
            .await
            .with_context(|| format!("requesting {repo_path}"))?
            .error_for_status()
            .with_context(|| format!("downloading {repo_path}"))?
            .bytes()
            .await
            .with_context(|| format!("reading {repo_path}"))?;
        std::fs::write(staging.path().join(rel), &bytes)
            .with_context(|| format!("writing {rel}"))?;
    }

    swap_into_place(staging, dir)
}

async fn list_hf_repo_files(
    client: &reqwest::Client,
    repo: &str,
    revision: &str,
    auth: Option<&str>,
) -> Result<Vec<String>> {
    let url = format!("{HF_API_BASE}/{repo}/revision/{revision}");
    let mut request = client.get(&url).header("User-Agent", "goose-ai-agent");
    if let Some(header) = auth {
        request = request.header("Authorization", header);
    }
    let response = request
        .send()
        .await
        .with_context(|| format!("listing files in {repo}"))?;
    if !response.status().is_success() {
        bail!(
            "Hugging Face API returned status {} for repo {repo}",
            response.status()
        );
    }

    #[derive(serde::Deserialize)]
    struct HfModel {
        #[serde(default)]
        siblings: Vec<HfSibling>,
    }
    #[derive(serde::Deserialize)]
    struct HfSibling {
        rfilename: String,
    }

    let model: HfModel = response.json().await.context("parsing HF repo listing")?;
    Ok(model.siblings.into_iter().map(|s| s.rfilename).collect())
}

async fn download_zip_and_install(
    url: &str,
    expected_sha256: Option<&str>,
    dir: &Path,
) -> Result<()> {
    let response = reqwest::get(url)
        .await
        .with_context(|| format!("requesting {url}"))?
        .error_for_status()
        .with_context(|| format!("downloading {url}"))?;
    let bytes = response.bytes().await.context("reading response body")?;

    if let Some(expected) = expected_sha256 {
        let actual = hex_sha256(&bytes);
        if !actual.eq_ignore_ascii_case(expected) {
            bail!("bundle checksum mismatch: expected {expected}, got {actual}");
        }
    } else {
        tracing::warn!(
            target: "goose::router",
            "no {BUNDLE_SHA256_KEY} set; installing router bundle without integrity verification",
        );
    }

    install_zip(&bytes, dir).context("installing bundle")
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Create a staging directory as a sibling of the final bundle directory so the
/// later `rename` into place is atomic (same filesystem).
fn new_staging_dir(dir: &Path) -> Result<tempfile::TempDir> {
    let parent = dir.parent().context("bundle directory has no parent")?;
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    tempfile::Builder::new()
        .prefix(".complexity_model.tmp")
        .tempdir_in(parent)
        .context("creating staging directory")
}

/// Atomically replace `dir` with the fully-populated `staging` directory.
fn swap_into_place(staging: tempfile::TempDir, dir: &Path) -> Result<()> {
    if dir.exists() {
        std::fs::remove_dir_all(dir)
            .with_context(|| format!("removing stale bundle at {}", dir.display()))?;
    }
    let staged = staging.keep();
    std::fs::rename(&staged, dir)
        .with_context(|| format!("installing bundle into {}", dir.display()))?;
    Ok(())
}

/// Extract a zip archive into `dir` atomically: unpack into a sibling temp
/// directory first, then swap it into place so a partial download never leaves
/// a half-written bundle that the loader would choke on.
fn install_zip(bytes: &[u8], dir: &Path) -> Result<()> {
    let staging = new_staging_dir(dir)?;

    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).context("opening zip archive")?;

    let mut saw_config = false;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(enclosed) = entry.enclosed_name() else {
            bail!("zip entry {} has an unsafe path", entry.name());
        };
        // Bundles may be zipped either flat or under a top-level directory;
        // strip a single leading component so files land directly in `dir`.
        let relative = strip_single_prefix(&enclosed);
        if relative.as_os_str().is_empty() {
            continue;
        }
        let out_path = staging.path().join(&relative);

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(p) = out_path.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf)?;
        std::fs::write(&out_path, &buf)
            .with_context(|| format!("writing {}", out_path.display()))?;
        if relative.file_name().and_then(|n| n.to_str()) == Some("config.json") {
            saw_config = true;
        }
    }

    if !saw_config {
        bail!("archive does not contain config.json; not a valid router bundle");
    }

    swap_into_place(staging, dir)
}

fn strip_single_prefix(path: &Path) -> std::path::PathBuf {
    let mut components = path.components();
    let first = components.next();
    // If the first component is a normal directory and there's more after it,
    // drop it (top-level archive folder); otherwise keep the whole path.
    match first {
        Some(std::path::Component::Normal(_)) if components.clone().next().is_some() => {
            components.as_path().to_path_buf()
        }
        _ => path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts = SimpleFileOptions::default();
            for (name, data) in entries {
                writer.start_file(*name, opts).unwrap();
                writer.write_all(data).unwrap();
            }
            writer.finish().unwrap();
        }
        buf
    }

    #[test]
    fn installs_flat_archive() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("complexity_model");
        let zip = make_zip(&[("config.json", b"{}"), ("weights.safetensors", b"abc")]);

        install_zip(&zip, &dir).unwrap();

        assert!(dir.join("config.json").exists());
        assert_eq!(
            std::fs::read(dir.join("weights.safetensors")).unwrap(),
            b"abc"
        );
    }

    #[test]
    fn installs_archive_with_top_level_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("complexity_model");
        let zip = make_zip(&[
            ("bundle/config.json", b"{}"),
            ("bundle/tokenizer.json", b"tok"),
        ]);

        install_zip(&zip, &dir).unwrap();

        assert!(dir.join("config.json").exists());
        assert_eq!(std::fs::read(dir.join("tokenizer.json")).unwrap(), b"tok");
    }

    #[test]
    fn rejects_archive_without_config() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("complexity_model");
        let zip = make_zip(&[("weights.safetensors", b"abc")]);

        assert!(install_zip(&zip, &dir).is_err());
    }

    #[test]
    fn replaces_existing_bundle() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("complexity_model");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("stale.txt"), b"old").unwrap();

        let zip = make_zip(&[("config.json", b"{}")]);
        install_zip(&zip, &dir).unwrap();

        assert!(dir.join("config.json").exists());
        assert!(!dir.join("stale.txt").exists());
    }

    #[test]
    fn hex_sha256_matches_known_vector() {
        // SHA-256 of "abc"
        assert_eq!(
            hex_sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
