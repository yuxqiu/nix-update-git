pub mod fetcher;
pub mod flake_input;
pub mod traits;

pub use fetcher::FetcherRule;
pub use flake_input::FlakeInputRule;
pub use traits::{RuleRegistry, Update, UpdateRule};
