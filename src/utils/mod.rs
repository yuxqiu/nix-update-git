pub mod fetch;
pub mod patch;
pub mod tarball;
pub mod version;

pub use fetch::{GitFetcher, GitRef, RefKind, RefType};
pub use nix_prefetch_git::NarHash;
pub use nix_prefetch_git::nar::flat_hash;
pub use nix_prefetch_git::nar::hash_path;
pub use patch::PatchHasher;
pub use tarball::TarballHasher;
pub use version::VersionDetector;
