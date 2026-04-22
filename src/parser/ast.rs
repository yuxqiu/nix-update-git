use rowan::ast::AstNode;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use anyhow::anyhow;

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
    node.text().to_string().trim().to_string()
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
        self.node.text().to_string().trim().to_string()
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

    // -- Private helpers ---------------------------------------------------

    /// Find the first direct child of a specific syntax kind.
    fn find_child(&self, kind: rnix::SyntaxKind) -> Option<NixNode> {
        self.children().into_iter().find(|c| c.kind() == kind)
    }

    /// Return the normalised interpolation parts of a `NODE_STRING`,
    /// or `None` if this is not a string node.
    fn string_parts(&self) -> Option<Vec<rnix::ast::InterpolPart<String>>> {
        if self.kind() != rnix::SyntaxKind::NODE_STRING {
            return None;
        }
        let nix_str = rnix::ast::Str::cast(self.node.clone())?;
        Some(nix_str.normalized_parts())
    }

    /// Search immediate-child `# comment` tokens.  For each comment,
    /// strip the leading `#` and surrounding whitespace, then call `f`.
    /// Returns `Some(R)` on the first `Some` returned by `f`, or
    /// `None` if no comment matched.
    fn find_comment<R>(&self, f: impl Fn(&str) -> Option<R>) -> Option<R> {
        for element in self.node.children_with_tokens() {
            if let rowan::NodeOrToken::Token(t) = element
                && t.kind() == rnix::SyntaxKind::TOKEN_COMMENT
            {
                let content = t.text().trim().trim_start_matches('#').trim();
                if let Some(result) = f(content) {
                    return Some(result);
                }
            }
        }
        None
    }

    // -- Public API -------------------------------------------------------

    pub fn has_pin_comment(&self) -> bool {
        self.find_comment(|c| c.starts_with("pin").then_some(()))
            .is_some()
    }

    pub fn attrpath_segments(&self) -> Vec<String> {
        if self.kind() != rnix::SyntaxKind::NODE_ATTRPATH_VALUE {
            return vec![];
        }
        self.find_child(rnix::SyntaxKind::NODE_ATTRPATH)
            .map(|node| {
                node.children()
                    .into_iter()
                    .filter(|c| c.kind() == rnix::SyntaxKind::NODE_IDENT)
                    .map(|c| c.text_trimmed())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn attr_value(&self) -> Option<NixNode> {
        if self.kind() != rnix::SyntaxKind::NODE_ATTRPATH_VALUE {
            return None;
        }
        self.children()
            .into_iter()
            .find(|c| VALUE_KINDS.contains(&c.kind()))
    }

    pub fn pure_string_content(&self) -> Option<String> {
        let parts = self.string_parts()?;
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
        let parts = self.string_parts()?;
        let mut result = String::new();
        for part in parts {
            match part {
                rnix::ast::InterpolPart::Literal(lit) => result.push_str(&lit),
                rnix::ast::InterpolPart::Interpolation(interpol) => {
                    let expr_text = interpol.expr()?.syntax().text().to_string();
                    let expr_trimmed = expr_text.trim();
                    result.push_str(vars.get(expr_trimmed)?);
                }
            }
        }
        Some(result)
    }

    /// Extract the prefix and suffix around a single occurrence of
    /// `var_name` in an interpolated string, resolving all other
    /// interpolations from `vars`.  For example, given
    /// `"${pname}-${version}"` with `var_name = "version"` and
    /// `vars = {"pname": "foo"}`, returns `("foo-", "")`.
    ///
    /// Pass an empty `vars` map when the string contains at most one
    /// interpolation (the target variable).
    ///
    /// Returns `None` if the target variable appears more than once,
    /// does not appear at all, or a non-target variable is not in
    /// `vars`.
    pub fn interpolated_var_affixes(
        &self,
        var_name: &str,
        vars: &HashMap<String, String>,
    ) -> Option<(String, String)> {
        let parts = self.string_parts()?;
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
                    let expr_trimmed = expr_text.trim();
                    if expr_trimmed == var_name {
                        if seen_var {
                            return None;
                        }
                        seen_var = true;
                    } else if let Some(value) = vars.get(expr_trimmed) {
                        if seen_var {
                            suffix.push_str(value);
                        } else {
                            prefix.push_str(value);
                        }
                    } else {
                        return None;
                    }
                }
            }
        }
        seen_var.then_some((prefix, suffix))
    }

    pub fn find_attr_by_key(&self, key: &str) -> Option<NixNode> {
        if self.kind() != rnix::SyntaxKind::NODE_ATTR_SET {
            return None;
        }
        self.children().into_iter().find(|child| {
            let segs = child.attrpath_segments();
            segs.len() == 1 && segs[0] == key
        })
    }

    pub fn find_string_value(&self, key: &str) -> Option<String> {
        self.find_attr_by_key(key)?
            .attr_value()?
            .pure_string_content()
    }

    pub fn find_string_node(&self, key: &str) -> Option<NixNode> {
        let value = self.find_attr_by_key(key)?.attr_value()?;
        (value.kind() == rnix::SyntaxKind::NODE_STRING).then_some(value)
    }

    pub fn find_bool_value(&self, key: &str) -> Option<bool> {
        let value = self.find_attr_by_key(key)?.attr_value()?;
        match value.kind() {
            rnix::SyntaxKind::NODE_IDENT => match value.text_trimmed().as_str() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn parse_attrs(
        &self,
        spec: &[AttrSpec],
        ident_vars: Option<&HashMap<String, String>>,
    ) -> Result<ParsedAttrs, anyhow::Error> {
        if self.kind() != rnix::SyntaxKind::NODE_ATTR_SET {
            return Ok(ParsedAttrs::default());
        }

        let known_keys: HashMap<&str, &AttrType> =
            spec.iter().map(|s| (s.key, &s.attr_type)).collect();

        let mut strings: HashMap<String, String> = HashMap::new();
        let mut bools: HashMap<String, bool> = HashMap::new();
        let mut ints: HashMap<String, i64> = HashMap::new();
        let mut list_strings: HashMap<String, Vec<NixNode>> = HashMap::new();
        let mut list_ints: HashMap<String, Vec<i64>> = HashMap::new();
        let mut string_nodes: HashMap<String, NixNode> = HashMap::new();
        let mut unknown_keys: Vec<String> = Vec::new();

        for child in self.children() {
            if child.kind() != rnix::SyntaxKind::NODE_ATTRPATH_VALUE {
                continue;
            }
            let segments = child.attrpath_segments();
            if segments.len() != 1 {
                continue;
            }
            let key = segments[0].clone();
            let Some(value) = child.attr_value() else {
                continue;
            };

            match known_keys.get(key.as_str()) {
                Some(attr_type) => {
                    let actual_kind = value.kind();
                    match attr_type {
                        AttrType::String => {
                            if actual_kind == rnix::SyntaxKind::NODE_STRING {
                                string_nodes.insert(key.clone(), value.clone());
                                if let Some(content) = value.pure_string_content() {
                                    strings.insert(key, content);
                                }
                            } else if actual_kind == rnix::SyntaxKind::NODE_IDENT {
                                let trimmed = value.text_trimmed();
                                if trimmed == "true" || trimmed == "false" {
                                    return Err(anyhow!(
                                        "key \"{}\" expected string but found bool",
                                        key
                                    ));
                                } else if let Some(iv) = ident_vars
                                    && let Some(resolved) = iv.get(trimmed.as_str())
                                {
                                    strings.insert(key, resolved.clone());
                                } else {
                                    return Err(anyhow!(
                                        "key \"{}\" expected string but found ident '{}'",
                                        key,
                                        trimmed
                                    ));
                                }
                            } else {
                                return Err(anyhow!(
                                    "key \"{}\" expected string but found {:?}",
                                    key,
                                    actual_kind
                                ));
                            }
                        }
                        AttrType::Bool => {
                            if actual_kind == rnix::SyntaxKind::NODE_IDENT {
                                let trimmed = value.text_trimmed();
                                match trimmed.as_str() {
                                    "true" => {
                                        bools.insert(key, true);
                                    }
                                    "false" => {
                                        bools.insert(key, false);
                                    }
                                    _ => {
                                        return Err(anyhow!(
                                            "key \"{}\" expected bool but found ident '{}'",
                                            key,
                                            trimmed
                                        ));
                                    }
                                }
                            } else {
                                return Err(anyhow!(
                                    "key \"{}\" expected bool but found {:?}",
                                    key,
                                    actual_kind
                                ));
                            }
                        }
                        AttrType::Int => {
                            if actual_kind == rnix::SyntaxKind::NODE_LITERAL {
                                let trimmed = value.text_trimmed();
                                if let Ok(num) = trimmed.parse::<i64>() {
                                    ints.insert(key, num);
                                } else {
                                    return Err(anyhow!(
                                        "key \"{}\" expected int but found non-numeric literal",
                                        key
                                    ));
                                }
                            } else {
                                return Err(anyhow!(
                                    "key \"{}\" expected int but found {:?}",
                                    key,
                                    actual_kind
                                ));
                            }
                        }
                        AttrType::ListString => {
                            if actual_kind == rnix::SyntaxKind::NODE_LIST {
                                let mut items = Vec::new();
                                for item in value.children() {
                                    if item.kind() == rnix::SyntaxKind::NODE_STRING {
                                        items.push(item);
                                    }
                                }
                                list_strings.insert(key.clone(), items);
                            } else {
                                return Err(anyhow!(
                                    "key \"{}\" expected list of strings but found {:?}",
                                    key,
                                    actual_kind
                                ));
                            }
                        }
                        AttrType::ListInt => {
                            if actual_kind == rnix::SyntaxKind::NODE_LIST {
                                let mut items = Vec::new();
                                for item in value.children() {
                                    if item.kind() == rnix::SyntaxKind::NODE_LITERAL {
                                        if let Ok(num) = item.text_trimmed().parse::<i64>() {
                                            items.push(num);
                                        } else {
                                            return Err(anyhow!(
                                                "key \"{}\" list contains non-integer item",
                                                key
                                            ));
                                        }
                                    } else {
                                        return Err(anyhow!(
                                            "key \"{}\" list contains non-integer item",
                                            key
                                        ));
                                    }
                                }
                                list_ints.insert(key, items);
                            } else {
                                return Err(anyhow!(
                                    "key \"{}\" expected list of ints but found {:?}",
                                    key,
                                    actual_kind
                                ));
                            }
                        }
                    }
                }
                None => {
                    // Unknown key: try to parse as string for maximum
                    // compatibility, but still report it as unknown.
                    if value.kind() == rnix::SyntaxKind::NODE_STRING {
                        string_nodes.insert(key.clone(), value.clone());
                        if let Some(content) = value.pure_string_content() {
                            strings.insert(key.clone(), content);
                        }
                    } else if value.kind() == rnix::SyntaxKind::NODE_IDENT {
                        let trimmed = value.text_trimmed();
                        if trimmed == "true" {
                            bools.insert(key.clone(), true);
                        } else if trimmed == "false" {
                            bools.insert(key.clone(), false);
                        }
                    }
                    unknown_keys.push(key);
                }
            }
        }

        Ok(ParsedAttrs {
            strings,
            bools,
            ints,
            list_strings,
            list_ints,
            string_nodes,
            unknown_keys,
        })
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

    pub fn follow_branch_comment(&self) -> Option<String> {
        self.find_comment(|c| c.strip_prefix("follow:").map(|s| s.trim().to_string()))
    }

    pub fn apply_function_name(&self) -> Option<String> {
        if self.kind() != rnix::SyntaxKind::NODE_APPLY {
            return None;
        }
        self.children()
            .into_iter()
            .find_map(|child| match child.kind() {
                rnix::SyntaxKind::NODE_IDENT => Some(child.text_trimmed()),
                rnix::SyntaxKind::NODE_SELECT => {
                    let parts = collect_select_path(&child.node);
                    (!parts.is_empty()).then(|| parts.join("."))
                }
                _ => None,
            })
    }

    pub fn apply_argument(&self) -> Option<NixNode> {
        if self.kind() != rnix::SyntaxKind::NODE_APPLY {
            return None;
        }
        self.find_child(rnix::SyntaxKind::NODE_ATTR_SET)
    }

    /// Find the argument attrset of a function application, unwrapping
    /// parenthesized lambda patterns.
    ///
    /// For `f { ... }` this returns the direct `NODE_ATTR_SET` child
    /// (same as `apply_argument`). For `f (x: { ... })` it unwraps
    /// the `NODE_PAREN` → `NODE_LAMBDA` → `NODE_ATTR_SET` layers.
    pub fn apply_argument_attrset(&self) -> Option<NixNode> {
        if self.kind() != rnix::SyntaxKind::NODE_APPLY {
            return None;
        }
        // Try direct attrset child: f { ... }
        if let Some(attr_set) = self.apply_argument() {
            return Some(attr_set);
        }
        // Try parenthesized lambda: f (x: { ... })
        let lambda = self
            .find_child(rnix::SyntaxKind::NODE_PAREN)?
            .find_child(rnix::SyntaxKind::NODE_LAMBDA)?;
        lambda.find_child(rnix::SyntaxKind::NODE_ATTR_SET)
    }

    /// If the argument to this function application is a parenthesized
    /// lambda `(param: { ... })`, return the lambda parameter name.
    /// Returns `None` if the argument is a direct attrset or if the
    /// pattern doesn't match.
    pub fn apply_lambda_param(&self) -> Option<String> {
        if self.kind() != rnix::SyntaxKind::NODE_APPLY {
            return None;
        }
        // Only applies when there's no direct attrset child
        if self.apply_argument().is_some() {
            return None;
        }
        let lambda = self
            .find_child(rnix::SyntaxKind::NODE_PAREN)?
            .find_child(rnix::SyntaxKind::NODE_LAMBDA)?;
        lambda
            .find_child(rnix::SyntaxKind::NODE_IDENT_PARAM)
            .map(|p| p.text_trimmed())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrType {
    String,
    Bool,
    Int,
    ListString,
    ListInt,
}

impl fmt::Display for AttrType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AttrType::String => write!(f, "string"),
            AttrType::Bool => write!(f, "bool"),
            AttrType::Int => write!(f, "int"),
            AttrType::ListString => write!(f, "list of strings"),
            AttrType::ListInt => write!(f, "list of ints"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttrSpec {
    pub key: &'static str,
    pub attr_type: AttrType,
}

#[derive(Debug, Default)]
pub struct ParsedAttrs {
    pub strings: HashMap<String, String>,
    pub bools: HashMap<String, bool>,
    pub ints: HashMap<String, i64>,
    pub list_strings: HashMap<String, Vec<NixNode>>,
    pub list_ints: HashMap<String, Vec<i64>>,
    pub string_nodes: HashMap<String, NixNode>,
    pub unknown_keys: Vec<String>,
}

impl ParsedAttrs {
    pub fn string_range(&self, key: &str) -> Option<TextRange> {
        self.string_nodes.get(key).map(|n| n.text_range())
    }

    pub fn has_string(&self, key: &str) -> bool {
        self.string_nodes.contains_key(key)
    }

    pub fn pure_string_list(&self, key: &str) -> Option<Vec<String>> {
        self.list_strings.get(key).and_then(|nodes| {
            let mut result = Vec::with_capacity(nodes.len());
            for n in nodes {
                result.push(n.pure_string_content()?);
            }
            Some(result)
        })
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
