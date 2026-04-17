use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use nix_prefetch_git::{NarHash, nar::hash_path};

pub struct TarballHasher;

impl TarballHasher {
    pub fn hash_tarball_url(url: &str) -> Result<NarHash> {
        let tarball_bytes = Self::download(url)?;
        let dir = tempfile::tempdir().context("Failed to create temp directory")?;
        Self::unpack_tarball(&tarball_bytes, dir.path())?;
        let content_dir = Self::find_content_dir(dir.path())?;
        hash_path(&content_dir).map_err(|e| anyhow::anyhow!("{e}"))
    }

    fn download(url: &str) -> Result<Vec<u8>> {
        let response = ureq::get(url)
            .call()
            .with_context(|| format!("Failed to download {}", url))?;
        let mut bytes = Vec::new();
        response
            .into_body()
            .into_reader()
            .read_to_end(&mut bytes)
            .context("Failed to read response body")?;
        Ok(bytes)
    }

    fn unpack_tarball(bytes: &[u8], dest: &Path) -> Result<()> {
        let gz = GzDecoder::new(bytes);
        let mut archive = tar::Archive::new(gz);
        archive
            .unpack(dest)
            .with_context(|| format!("Failed to unpack tarball to {}", dest.display()))?;
        Ok(())
    }

    fn find_content_dir(parent: &Path) -> Result<PathBuf> {
        let mut entries =
            fs::read_dir(parent).with_context(|| format!("Failed to read {}", parent.display()))?;
        let entry = entries
            .next()
            .context("Tarball produced no top-level entries")?;
        let first_path = entry.context("Failed to read directory entry")?.path();
        if entries.next().is_none() && first_path.is_dir() {
            // Single top-level directory (e.g. GitHub's "repo-1.0/"):
            // this is the content root, matching fetchzip's default stripRoot=true.
            Ok(first_path)
        } else {
            // Multiple entries or a single file at the top level (e.g. Gitiles
            // flat archives with stripRoot=false): the parent directory itself
            // is the content root.
            Ok(parent.to_path_buf())
        }
    }
}
