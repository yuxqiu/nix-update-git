use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use flate2::read::GzDecoder;

use super::nar::{NarHash, hash_path};

pub struct TarballHasher;

impl TarballHasher {
    pub fn hash_tarball_url(url: &str) -> Result<NarHash> {
        let tarball_bytes = Self::download(url)?;
        let dir = tempfile::tempdir().context("Failed to create temp directory")?;
        Self::unpack_tarball(&tarball_bytes, dir.path())?;
        let content_dir = Self::find_single_subdir(dir.path())?;
        hash_path(&content_dir)
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

    fn find_single_subdir(parent: &Path) -> Result<PathBuf> {
        let mut entries =
            fs::read_dir(parent).with_context(|| format!("Failed to read {}", parent.display()))?;
        let entry = entries
            .next()
            .context("Tarball produced no top-level directory")?;
        let path = entry.context("Failed to read directory entry")?.path();
        if entries.next().is_some() {
            anyhow::bail!("Tarball produced multiple top-level entries");
        }
        Ok(path)
    }
}
