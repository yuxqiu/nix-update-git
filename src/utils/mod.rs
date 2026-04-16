pub mod fetch;
pub mod tarball;
pub mod version;

pub use fetch::{GitFetcher, GitRef, RefKind, RefType};
pub use nix_prefetch_git::NarHash;
pub use nix_prefetch_git::nar::hash_path;
pub use tarball::TarballHasher;
pub use version::VersionDetector;
