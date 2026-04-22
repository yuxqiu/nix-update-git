mod ast;
mod tests;

pub use ast::{
    AttrSpec, AttrType, NixError, NixFile, NixNode, NixNodeIterator, ParsedAttrs, TextRange,
};
