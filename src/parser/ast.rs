use rowan::ast::AstNode;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum NixError {
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid node: {0}")]
    InvalidNode(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub file: Arc<Path>,
    pub line: usize,
    pub column: usize,
}

impl std::fmt::Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.file.display(), self.line, self.column)
    }
}

pub struct NixFile {
    path: Arc<Path>,
    root: rnix::Root,
    source: Arc<str>,
}

impl NixFile {
    pub fn parse(path: &Path, content: &str) -> Result<Self, NixError> {
        let parse_result = rnix::Root::parse(content);
        if !parse_result.errors().is_empty() {
            return Err(NixError::ParseError(format!(
                "{:?}",
                parse_result.errors().first().unwrap()
            )));
        }
        let root = parse_result.tree();
        Ok(Self {
            path: Arc::from(path),
            root,
            source: Arc::from(content),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn root(&self) -> &rnix::Root {
        &self.root
    }

    pub fn syntax(&self) -> &rnix::SyntaxNode {
        self.root.syntax()
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn root_node(&self) -> NixNode {
        NixNode::new(self.syntax().clone(), &self.path, self.source.clone())
    }
}

#[derive(Clone, Debug)]
pub struct NixNode {
    node: rnix::SyntaxNode,
    location: Location,
    source: Arc<str>,
}

impl NixNode {
    pub fn new(node: rnix::SyntaxNode, path: &Path, source: Arc<str>) -> Self {
        let location = Location {
            file: Arc::from(path),
            line: 1,
            column: 1,
        };
        Self {
            node,
            location,
            source,
        }
    }

    pub fn location(&self) -> Location {
        self.location.clone()
    }

    pub fn kind(&self) -> rnix::SyntaxKind {
        self.node.kind()
    }

    pub fn text(&self) -> String {
        self.node.text().to_string()
    }

    pub fn children(&self) -> Vec<NixNode> {
        self.node
            .children()
            .map(|child| NixNode::new(child, &self.location.file, self.source.clone()))
            .collect()
    }

    pub fn traverse(&self) -> NixNodeIterator {
        NixNodeIterator {
            stack: vec![self.clone()],
        }
    }

    pub fn syntax(&self) -> &rnix::SyntaxNode {
        &self.node
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn has_pin_comment(&self) -> bool {
        Self::check_pin_in_node(&self.node)
    }

    fn check_pin_in_node(node: &rnix::SyntaxNode) -> bool {
        for element in node.children_with_tokens() {
            match element {
                rowan::NodeOrToken::Token(t) => {
                    if t.kind() == rnix::SyntaxKind::TOKEN_COMMENT {
                        let text = t.text().trim();
                        if text.trim_start_matches('#').trim().starts_with("pin") {
                            return true;
                        }
                    }
                }
                rowan::NodeOrToken::Node(n) => {
                    if Self::check_pin_in_node(&n) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn attrpath_segments(&self) -> Vec<String> {
        if self.kind() != rnix::SyntaxKind::NODE_ATTRPATH_VALUE {
            return vec![];
        }
        for child in self.node.children() {
            if child.kind() == rnix::SyntaxKind::NODE_ATTRPATH {
                return child
                    .children()
                    .filter(|c| c.kind() == rnix::SyntaxKind::NODE_IDENT)
                    .map(|c| c.text().to_string().trim().to_string())
                    .collect();
            }
        }
        vec![]
    }

    pub fn attr_value(&self) -> Option<NixNode> {
        if self.kind() != rnix::SyntaxKind::NODE_ATTRPATH_VALUE {
            return None;
        }
        let value_kinds = [
            rnix::SyntaxKind::NODE_STRING,
            rnix::SyntaxKind::NODE_ATTR_SET,
            rnix::SyntaxKind::NODE_LIST,
            rnix::SyntaxKind::NODE_LITERAL,
            rnix::SyntaxKind::NODE_IDENT,
            rnix::SyntaxKind::NODE_APPLY,
            rnix::SyntaxKind::NODE_SELECT,
            rnix::SyntaxKind::NODE_PAREN,
            rnix::SyntaxKind::NODE_UNARY_OP,
            rnix::SyntaxKind::NODE_BIN_OP,
            rnix::SyntaxKind::NODE_IF_ELSE,
            rnix::SyntaxKind::NODE_WITH,
            rnix::SyntaxKind::NODE_ASSERT,
            rnix::SyntaxKind::NODE_LET_IN,
            rnix::SyntaxKind::NODE_LAMBDA,
        ];
        for child in self.node.children() {
            if value_kinds.contains(&child.kind()) {
                return Some(NixNode::new(
                    child,
                    &self.location.file,
                    self.source.clone(),
                ));
            }
        }
        None
    }

    pub fn string_content(&self) -> Option<String> {
        if self.kind() != rnix::SyntaxKind::NODE_STRING {
            return None;
        }
        let text = self.text();
        if text.starts_with("''") && text.ends_with("''") && text.len() >= 4 {
            Some(text[2..text.len() - 2].to_string())
        } else if text.starts_with('"') && text.ends_with('"') && text.len() >= 2 {
            Some(text[1..text.len() - 1].to_string())
        } else {
            Some(text)
        }
    }

    pub fn find_attr_by_key(&self, key: &str) -> Option<NixNode> {
        if self.kind() != rnix::SyntaxKind::NODE_ATTR_SET {
            return None;
        }
        for child in self.children() {
            if child.kind() == rnix::SyntaxKind::NODE_ATTRPATH_VALUE {
                let segments = child.attrpath_segments();
                if segments.len() == 1 && segments[0] == key {
                    return Some(child);
                }
            }
        }
        None
    }

    pub fn find_string_value(&self, key: &str) -> Option<String> {
        let entry = self.find_attr_by_key(key)?;
        let value = entry.attr_value()?;
        value.string_content()
    }

    pub fn find_attr_set(&self, key: &str) -> Option<NixNode> {
        let entry = self.find_attr_by_key(key)?;
        let value = entry.attr_value()?;
        if value.kind() == rnix::SyntaxKind::NODE_ATTR_SET {
            Some(value)
        } else {
            None
        }
    }

    pub fn text_range(&self) -> (usize, usize) {
        let range = self.node.text_range();
        (usize::from(range.start()), usize::from(range.end()))
    }

    pub fn find_string_node(&self, key: &str) -> Option<NixNode> {
        let entry = self.find_attr_by_key(key)?;
        let value = entry.attr_value()?;
        if value.kind() == rnix::SyntaxKind::NODE_STRING {
            Some(value)
        } else {
            None
        }
    }

    pub fn attr_set_entries(&self) -> HashMap<String, NixNode> {
        let mut entries = HashMap::new();
        if self.kind() != rnix::SyntaxKind::NODE_ATTR_SET {
            return entries;
        }
        for child in self.children() {
            if child.kind() == rnix::SyntaxKind::NODE_ATTRPATH_VALUE {
                let segments = child.attrpath_segments();
                if !segments.is_empty() {
                    let key = segments.join(".");
                    entries.insert(key, child);
                }
            }
        }
        entries
    }
}

#[derive(Debug)]
pub struct NixNodeIterator {
    stack: Vec<NixNode>,
}

impl Iterator for NixNodeIterator {
    type Item = NixNode;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.stack.pop()?;
        for child in node.children().into_iter().rev() {
            self.stack.push(child);
        }
        Some(node)
    }
}

impl IntoIterator for &NixFile {
    type Item = NixNode;
    type IntoIter = NixNodeIterator;

    fn into_iter(self) -> Self::IntoIter {
        NixNodeIterator {
            stack: vec![NixNode::new(
                self.syntax().clone(),
                self.path(),
                self.source.clone(),
            )],
        }
    }
}
