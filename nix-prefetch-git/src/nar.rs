//! NAR serialization and SHA-256 hashing.
//!
//! Produces the same hash as `nix-store --hash-path` / `nix-hash --type sha256 --base32`,
//! so it can be used as a drop-in replacement for the NAR-hashing step in
//! `nix-prefetch-git`.

use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use nix_nar::Encoder;
use sha2::{Digest, Sha256};

/// A SHA-256 NAR hash in multiple output formats.
#[derive(Debug, Clone)]
pub struct NarHash {
    /// SRI-format hash, e.g. `"sha256-RTt0byvWGFjqkJXE1t1DjWlJqmE0rq94KOGojukeD6M="`.
    pub sri: String,
    /// Nix-base32 hash, e.g. `"0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n"`.
    pub nix32: String,
}

/// Compute the NAR SHA-256 hash of a filesystem path.
///
/// This serialises the path into the NAR format and hashes the resulting
/// byte stream with SHA-256. The result is returned in both SRI and
/// nix-base32 formats.
pub fn hash_path(path: &Path) -> crate::Result<NarHash> {
    let mut encoder = Encoder::new(path).map_err(|e| crate::Error::NarHash {
        path: path.display().to_string(),
        source: std::io::Error::other(e),
    })?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut encoder, &mut hasher).map_err(|e| crate::Error::NarHash {
        path: path.display().to_string(),
        source: e,
    })?;
    Ok(digest_to_nar_hash(&hasher.finalize()))
}

/// Compute a flat SHA-256 hash of raw bytes.
///
/// This produces the same hash as `outputHashMode = "flat"` in Nix,
/// which simply hashes the file content directly (not NAR-serialized).
/// Used by `fetchurl` and `fetchpatch` which use flat hashing.
pub fn flat_hash(content: &[u8]) -> NarHash {
    let digest = Sha256::digest(content);
    NarHash {
        sri: format!("sha256-{}", STANDARD.encode(digest)),
        nix32: nix_base32::to_nix_base32(&digest),
    }
}

fn digest_to_nar_hash(digest: &[u8]) -> NarHash {
    NarHash {
        sri: format!("sha256-{}", STANDARD.encode(digest)),
        nix32: nix_base32::to_nix_base32(digest),
    }
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

        assert_eq!(
            result.sri,
            "sha256-RTt0byvWGFjqkJXE1t1DjWlJqmE0rq94KOGojukeD6M="
        );
        assert!(!result.nix32.is_empty());
    }

    #[test]
    fn test_hash_deterministic() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("file.txt"), "content\n").unwrap();

        let h1 = hash_path(dir.path()).unwrap();
        let h2 = hash_path(dir.path()).unwrap();

        assert_eq!(h1.sri, h2.sri);
        assert_eq!(h1.nix32, h2.nix32);
    }

    #[test]
    fn test_hash_empty_directory() {
        let dir = tempfile::tempdir().unwrap();

        let result = hash_path(dir.path()).unwrap();

        assert!(!result.sri.is_empty());
        assert!(!result.nix32.is_empty());
    }

    #[test]
    fn test_hash_nested_directory() {
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("subdir");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join("nested.txt"), "nested content\n").unwrap();
        fs::write(dir.path().join("top.txt"), "top level\n").unwrap();

        let result = hash_path(dir.path()).unwrap();

        assert!(!result.sri.is_empty());
        assert!(!result.nix32.is_empty());
    }

    #[test]
    fn test_sri_format_is_valid_base64() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("data.bin"), b"\x00\x01\x02\xff").unwrap();

        let result = hash_path(dir.path()).unwrap();

        // SRI format should be "sha256-<base64>"
        let b64 = result.sri.strip_prefix("sha256-").unwrap();
        let decoded = STANDARD.decode(b64).unwrap();
        assert_eq!(decoded.len(), 32); // SHA-256 is 32 bytes
    }

    #[test]
    fn test_nix32_format_round_trips() {
        // nix-base32 encoding of a 32-byte hash should produce a string
        // that is always the same length for SHA-256 hashes.
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "a\n").unwrap();

        let result = hash_path(dir.path()).unwrap();

        // nix-base32 SHA-256 hashes are always 52 characters.
        assert_eq!(result.nix32.len(), 52);
    }
}
