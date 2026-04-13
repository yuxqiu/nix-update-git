use std::path::Path;

use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use nix_base32;
use nix_nar::Encoder;
use sha2::{Digest, Sha256};

pub struct NarHash {
    pub sri: String,
    pub nix32: String,
    pub hex: String,
}

pub fn hash_path(path: &Path) -> Result<NarHash> {
    let mut encoder = Encoder::new(path)
        .with_context(|| format!("Failed to create NAR encoder for {}", path.display()))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut encoder, &mut hasher)
        .with_context(|| format!("Failed to NAR-serialize {}", path.display()))?;
    Ok(digest_to_nar_hash(&hasher.finalize()))
}

fn digest_to_nar_hash(digest: &[u8]) -> NarHash {
    NarHash {
        sri: format!("sha256-{}", STANDARD.encode(digest)),
        nix32: nix_base32::to_nix_base32(digest),
        hex: hex_encode(digest),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_hash_simple_directory() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("hello.txt"), "hello world\n").unwrap();

        let result = hash_path(dir.path()).unwrap();

        dbg!(dir);
        dbg!(&result.sri);

        assert!(result.sri == "sha256-RTt0byvWGFjqkJXE1t1DjWlJqmE0rq94KOGojukeD6M=");
        assert!(!result.nix32.is_empty());
        assert_eq!(result.hex.len(), 64);
    }

    #[test]
    fn test_hash_deterministic() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("file.txt"), "content\n").unwrap();

        let h1 = hash_path(dir.path()).unwrap();
        let h2 = hash_path(dir.path()).unwrap();

        assert_eq!(h1.sri, h2.sri);
        assert_eq!(h1.nix32, h2.nix32);
        assert_eq!(h1.hex, h2.hex);
    }
}
