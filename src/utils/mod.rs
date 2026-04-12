pub mod fetch;
pub mod prefetch;
pub mod version;

pub use fetch::{GitFetcher, GitRef, RefType};
pub use prefetch::NixPrefetcher;
pub use version::VersionDetector;
