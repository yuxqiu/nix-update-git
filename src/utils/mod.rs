pub mod fetch;
pub mod version;

pub use fetch::{GitFetcher, GitRef};
pub use version::VersionDetector;
