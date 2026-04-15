use rowan::ast::AstNode;
use std::collections::HashMap;
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
    pub fn parse(content: &str) -> Result<Self, NixError> {
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
        NixNode::new(self.root.syntax().clone(), self.source.clone())
    }
}

const VALUE_KINDS: [rnix::SyntaxKind; 15] = [
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

fn syntax_node_text_trimmed(node: &rnix::SyntaxNode) -> String {
    let s = node.text().to_string();
    s.trim().to_string()
}

fn collect_select_path(node: &rnix::SyntaxNode) -> Vec<String> {
    let mut parts = Vec::new();

    fn collect_from_attrpath(node: &rnix::SyntaxNode, parts: &mut Vec<String>) {
        for child in node.children() {
            if child.kind() == rnix::SyntaxKind::NODE_IDENT {
                parts.push(syntax_node_text_trimmed(&child));
            } else if child.kind() == rnix::SyntaxKind::NODE_SELECT {
                collect_select_parts(&child, parts);
            }
        }
    }

    fn collect_select_parts(node: &rnix::SyntaxNode, parts: &mut Vec<String>) {
        let children: Vec<_> = node.children().collect();
        if children.len() >= 2 {
            let first = &children[0];
            let last = &children[children.len() - 1];

            if first.kind() == rnix::SyntaxKind::NODE_SELECT {
                collect_select_parts(first, parts);
            } else if first.kind() == rnix::SyntaxKind::NODE_IDENT {
                parts.push(syntax_node_text_trimmed(first));
            }

            if last.kind() == rnix::SyntaxKind::NODE_ATTRPATH {
                collect_from_attrpath(last, parts);
            } else if last.kind() == rnix::SyntaxKind::NODE_IDENT {
                parts.push(syntax_node_text_trimmed(last));
            }
        }
    }

    collect_select_parts(node, &mut parts);
    parts
}

#[derive(Clone, Debug)]
pub struct NixNode {
    node: rnix::SyntaxNode,
    source: Arc<str>,
}

impl NixNode {
    pub fn new(node: rnix::SyntaxNode, source: Arc<str>) -> Self {
        Self { node, source }
    }

    pub fn kind(&self) -> rnix::SyntaxKind {
        self.node.kind()
    }

    pub fn text(&self) -> String {
        self.node.text().to_string()
    }

    pub fn text_trimmed(&self) -> String {
        let s = self.node.text().to_string();
        s.trim().to_string()
    }

    pub fn children(&self) -> Vec<NixNode> {
        self.node
            .children()
            .map(|child| NixNode::new(child, self.source.clone()))
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
                    .map(|c| syntax_node_text_trimmed(&c))
                    .collect();
            }
        }
        vec![]
    }

    pub fn attr_value(&self) -> Option<NixNode> {
        if self.kind() != rnix::SyntaxKind::NODE_ATTRPATH_VALUE {
            return None;
        }
        for child in self.node.children() {
            if VALUE_KINDS.contains(&child.kind()) {
                return Some(NixNode::new(child, self.source.clone()));
            }
        }
        None
    }

    pub fn pure_string_content(&self) -> Option<String> {
        if self.kind() != rnix::SyntaxKind::NODE_STRING {
            return None;
        }
        let nix_str = rnix::ast::Str::cast(self.node.clone())?;
        let parts = nix_str.normalized_parts();
        let mut result = String::new();
        for part in parts {
            match part {
                rnix::ast::InterpolPart::Literal(lit) => result.push_str(&lit),
                rnix::ast::InterpolPart::Interpolation(_) => return None,
            }
        }
        Some(result)
    }

    pub fn interpolated_string_content(&self, vars: &HashMap<String, String>) -> Option<String> {
        if self.kind() != rnix::SyntaxKind::NODE_STRING {
            return None;
        }
        let nix_str = rnix::ast::Str::cast(self.node.clone())?;
        let parts = nix_str.normalized_parts();
        let mut result = String::new();
        for part in parts {
            match part {
                rnix::ast::InterpolPart::Literal(lit) => result.push_str(&lit),
                rnix::ast::InterpolPart::Interpolation(interpol) => {
                    let expr_text = interpol.expr()?.syntax().text().to_string();
                    let expr_trimmed = expr_text.trim();
                    let value = vars.get(expr_trimmed)?;
                    result.push_str(value);
                }
            }
        }
        Some(result)
    }

    pub fn interpolated_single_var_affixes(&self, var_name: &str) -> Option<(String, String)> {
        if self.kind() != rnix::SyntaxKind::NODE_STRING {
            return None;
        }
        let nix_str = rnix::ast::Str::cast(self.node.clone())?;
        let parts = nix_str.normalized_parts();
        let mut seen_var = false;
        let mut prefix = String::new();
        let mut suffix = String::new();
        for part in parts {
            match part {
                rnix::ast::InterpolPart::Literal(lit) => {
                    if seen_var {
                        suffix.push_str(&lit);
                    } else {
                        prefix.push_str(&lit);
                    }
                }
                rnix::ast::InterpolPart::Interpolation(interpol) => {
                    let expr_text = interpol.expr()?.syntax().text().to_string();
                    if expr_text.trim() != var_name || seen_var {
                        return None;
                    }
                    seen_var = true;
                }
            }
        }
        if !seen_var {
            return None;
        }
        Some((prefix, suffix))
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
        value.pure_string_content()
    }

    pub fn parent(&self) -> Option<NixNode> {
        self.node
            .parent()
            .map(|p| NixNode::new(p, self.source.clone()))
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

    pub fn follow_branch_comment(&self) -> Option<String> {
        for element in self.node.children_with_tokens() {
            if let rowan::NodeOrToken::Token(t) = element
                && t.kind() == rnix::SyntaxKind::TOKEN_COMMENT
            {
                let text = t.text().trim();
                let content = text.trim_start_matches('#').trim();
                if let Some(branch) = content.strip_prefix("follow:") {
                    return Some(branch.trim().to_string());
                }
            }
        }
        None
    }

    pub fn apply_function_name(&self) -> Option<String> {
        if self.kind() != rnix::SyntaxKind::NODE_APPLY {
            return None;
        }
        for child in self.children() {
            if child.kind() == rnix::SyntaxKind::NODE_IDENT {
                return Some(child.text_trimmed());
            }
            if child.kind() == rnix::SyntaxKind::NODE_SELECT {
                let parts = collect_select_path(&child.node);
                if !parts.is_empty() {
                    return Some(parts.join("."));
                }
            }
        }
        None
    }

    pub fn apply_argument(&self) -> Option<NixNode> {
        if self.kind() != rnix::SyntaxKind::NODE_APPLY {
            return None;
        }
        self.children()
            .into_iter()
            .find(|child| child.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
    }

    pub fn find_bool_value(&self, key: &str) -> Option<bool> {
        let entry = self.find_attr_by_key(key)?;
        let value = entry.attr_value()?;
        if value.kind() == rnix::SyntaxKind::NODE_IDENT {
            let trimmed = value.text_trimmed();
            match trimmed.as_str() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            }
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
