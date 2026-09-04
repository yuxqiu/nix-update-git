mod color;
mod diff;
mod text;

pub use color::{new_line, old_line};
pub use diff::{FileDiff, Hunk, LineChange};
pub use text::render;
