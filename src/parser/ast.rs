use rowan::ast::AstNode;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug)]
pub enum NixError {
    ParseError(String),
    InvalidNode(String),
}

impl std::fmt::Display for NixError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NixError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            NixError::InvalidNode(msg) => write!(f, "Invalid node: {}", msg),
        }
    }
}

impl std::error::Error for NixError {}

pub struct NixFile {
    root: rnix::Root,
    source: Arc<str>,
}

impl NixFile {
    pub fn parse(_path: &Path, content: &str) -> Result<Self, NixError> {
        let parse_result = rnix::Root::parse(content);
        if !parse_result.errors().is_empty() {
            let error_msgs: Vec<String> = parse_result
                .errors()
                .iter()
                .map(|e| e.to_string())
                .collect();
            return Err(NixError::ParseError(error_msgs.join(", ")));
        }
        let root = parse_result.tree();
        Ok(Self {
            root,
            source: Arc::from(content),
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn root_node(&self) -> NixNode {
        NixNode::new(
            self.root.syntax().clone(),
            Arc::from(Path::new("")),
            self.source.clone(),
        )
    }
}

#[derive(Clone, Debug)]
pub struct NixNode {
    node: rnix::SyntaxNode,
    file: Arc<Path>,
    source: Arc<str>,
}

impl NixNode {
    pub fn new(node: rnix::SyntaxNode, file: Arc<Path>, source: Arc<str>) -> Self {
        Self { node, file, source }
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
            .map(|child| NixNode::new(child, self.file.clone(), self.source.clone()))
            .collect()
    }

    pub fn traverse(&self) -> NixNodeIterator {
        NixNodeIterator {
            stack: vec![self.clone()],
        }
    }

    pub fn has_pin_comment(&self) -> bool {
        for element in self.node.children_with_tokens() {
            if let rowan::NodeOrToken::Token(t) = element
                && t.kind() == rnix::SyntaxKind::TOKEN_COMMENT
            {
                let text = t.text().trim();
                if text.trim_start_matches('#').trim().starts_with("pin") {
                    return true;
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
                return Some(NixNode::new(child, self.file.clone(), self.source.clone()));
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
            None
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

    pub fn text_range(&self) -> TextRange {
        let range = self.node.text_range();
        TextRange {
            start: usize::from(range.start()),
            end: usize::from(range.end()),
        }
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
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
