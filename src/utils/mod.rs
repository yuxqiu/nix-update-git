pub mod fetch;
pub mod prefetch;
pub mod version;

pub use fetch::{GitFetcher, GitRef, RefKind, RefType};
pub use prefetch::NixPrefetcher;
pub use version::VersionDetector;
