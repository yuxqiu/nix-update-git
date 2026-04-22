use std::collections::HashMap;

use anyhow::Result;

use crate::parser::{NixNode, ParsedAttrs};
use crate::rules::traits::{Update, UpdateRule};
use crate::utils::{GitFetcher, NarHash, VersionDetector};

use kind::{FetcherKind, HashStrategy};

pub mod git_fetch;
pub mod kind;
pub mod patch_url;
pub mod tarball;

pub(crate) fn is_commit_hash(s: &str) -> bool {
    s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit())
}

pub(crate) fn version_ref_key_and_value(
    kind: FetcherKind,
    parsed: &ParsedAttrs,
) -> Option<(&'static str, String)> {
    if let Some(tag) = parsed.strings.get("tag") {
        return Some(("tag", tag.clone()));
    }
    if let Some(rev) = parsed.strings.get("rev") {
        if is_commit_hash(rev) || !VersionDetector::is_version(rev) {
            return None;
        }
        return Some(("rev", rev.clone()));
    }
    if let Some(ref_val) = parsed.strings.get("ref")
        && kind == FetcherKind::BuiltinsFetchGit
    {
        if is_commit_hash(ref_val) || !VersionDetector::is_version(ref_val) {
            return None;
        }
        return Some(("ref", ref_val.clone()));
    }
    None
}

pub(crate) fn preferred_ref_key(parsed: &ParsedAttrs) -> Option<&'static str> {
    if parsed.strings.contains_key("tag") {
        Some("tag")
    } else if parsed.strings.contains_key("rev") {
        Some("rev")
    } else if parsed.strings.contains_key("ref") {
        Some("ref")
    } else {
        None
    }
}

pub(crate) fn resolve_ref_for_prefetch(_git_url: &str, ref_value: &str) -> Option<String> {
    if ref_value.is_empty() {
        return None;
    }
    Some(ref_value.to_string())
}

pub(crate) struct InterpolationSpec {
    allowed: HashMap<String, HashMap<String, String>>,
    allow_all_vars: Option<HashMap<String, String>>,
    ident_vars: HashMap<String, String>,
}

impl InterpolationSpec {
    pub(crate) fn none() -> Self {
        Self {
            allowed: HashMap::new(),
            allow_all_vars: None,
            ident_vars: HashMap::new(),
        }
    }

    pub(crate) fn allow(&mut self, field: &str, vars: HashMap<String, String>) {
        self.allowed.insert(field.to_string(), vars);
    }

    pub(crate) fn allow_all(&mut self, vars: HashMap<String, String>) {
        self.allow_all_vars = Some(vars);
    }

    pub(crate) fn allow_idents(&mut self, idents: HashMap<String, String>) {
        self.ident_vars = idents;
    }

    pub(crate) fn vars_for_field(&self, field: &str) -> Option<HashMap<String, String>> {
        match (&self.allow_all_vars, self.allowed.get(field)) {
            (None, None) => None,
            (None, Some(field_vars)) => Some(field_vars.clone()),
            (Some(default_vars), None) => Some(default_vars.clone()),
            (Some(default_vars), Some(field_vars)) => {
                let mut merged = default_vars.clone();
                merged.extend(field_vars.iter().map(|(k, v)| (k.clone(), v.clone())));
                Some(merged)
            }
        }
    }
}

pub(crate) struct FetcherAttrs {
    pub parsed: ParsedAttrs,
    pub interpolated: HashMap<String, NixNode>,
    pub interpolated_unresolved: Vec<String>,
}

pub(crate) fn parse_fetcher_attrset(
    kind: FetcherKind,
    attr_set: &NixNode,
    spec: &InterpolationSpec,
) -> Result<FetcherAttrs, anyhow::Error> {
    let ident_vars_opt = if spec.ident_vars.is_empty() {
        None
    } else {
        Some(&spec.ident_vars)
    };
    let parsed = attr_set.parse_attrs(kind.attr_spec(), ident_vars_opt)?;

    let mut interpolated = HashMap::new();
    let mut interpolated_unresolved = Vec::new();

    for (key, node) in &parsed.string_nodes {
        if node.pure_string_content().is_some() {
            // Already handled by parse_attrs -> strings
        } else if let Some(vars) = spec.vars_for_field(key) {
            if node.interpolated_string_content(&vars).is_some() {
                interpolated.insert(key.clone(), node.clone());
            } else {
                interpolated_unresolved.push(key.clone());
            }
        } else {
            interpolated_unresolved.push(key.clone());
        }
    }

    Ok(FetcherAttrs {
        parsed,
        interpolated,
        interpolated_unresolved,
    })
}

struct FetcherCall {
    kind: FetcherKind,
    parsed: ParsedAttrs,
    pinned: bool,
    follow_branch: Option<String>,
}

#[derive(Default)]
pub struct FetcherRule;

impl FetcherRule {
    fn try_extract_call(node: &NixNode) -> Option<FetcherCall> {
        let func_name = node.apply_function_name()?;
        let kind = FetcherKind::from_name(&func_name)?;
        let arg = node.apply_argument()?;

        if arg.kind() != rnix::SyntaxKind::NODE_ATTR_SET {
            return None;
        }

        let op_keys = kind.operational_keys();
        let attrs = match parse_fetcher_attrset(kind, &arg, &InterpolationSpec::none()) {
            Ok(a) => a,
            Err(_) => return None,
        };

        if attrs
            .interpolated_unresolved
            .iter()
            .any(|k| op_keys.contains(&k.as_str()))
        {
            return None;
        }

        let pinned = arg.has_pin_comment() || node.has_pin_comment();
        let follow_branch = arg
            .follow_branch_comment()
            .or_else(|| node.follow_branch_comment());

        Some(FetcherCall {
            kind,
            parsed: attrs.parsed,
            pinned,
            follow_branch,
        })
    }

    fn check_fetcher_call(&self, call: &FetcherCall) -> Result<Option<Vec<Update>>> {
        let git_url = match call.kind.git_url(&call.parsed) {
            Some(url) => url,
            None => return Ok(None),
        };

        let mut updates = Vec::new();
        let mut version_updated_rev: Option<String> = None;

        if !call.pinned {
            if let Some(branch) = &call.follow_branch {
                version_updated_rev =
                    self.handle_branch_following(call, &git_url, branch, &mut updates)?;
            } else {
                version_updated_rev = self.handle_version_update(call, &git_url, &mut updates)?;
            }
        }

        if call.kind.needs_hash() {
            if let Some(rev) = &version_updated_rev {
                Self::try_prefetch_hash(call, rev, &mut updates);
            } else {
                Self::try_prefetch_empty_hash(call, &git_url, &mut updates);
            }
        }

        if updates.is_empty() {
            Ok(None)
        } else {
            Ok(Some(updates))
        }
    }

    fn check_fetchpatch_call(call: &FetcherCall) -> Result<Option<Vec<Update>>> {
        let url = match call.parsed.strings.get("url") {
            Some(url) => url,
            None => return Ok(None),
        };

        let mut updates = Vec::new();
        let mut current_url = url.clone();
        let mut url_changed = false;

        let parsed_url = patch_url::parse_patch_url(url);

        if let Some(branch) = &call.follow_branch {
            if let Some(parsed) = &parsed_url {
                let git_url = parsed.git_remote_url();
                match GitFetcher::get_latest_commit(&git_url, branch) {
                    Ok(Some(new_sha)) => {
                        let current_ref = parsed.current_ref();
                        if current_ref != new_sha {
                            current_url = parsed.replace_ref(&new_sha);
                            url_changed = true;
                        }
                    }
                    Ok(None) => {
                        eprintln!(
                            "Warning: could not find branch '{}' for {}",
                            branch, git_url
                        );
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning: could not fetch latest commit for {}: {:#}",
                            git_url, e
                        );
                    }
                }
            }
        } else if !call.pinned
            && let Some(parsed) = &parsed_url
            && parsed.is_version_ref()
        {
            let git_url = parsed.git_remote_url();
            let current = parsed.current_ref();
            if let Ok(Some(latest)) = GitFetcher::get_latest_tag_matching(&git_url, Some(current))
                && VersionDetector::compare(current, &latest) == std::cmp::Ordering::Less
            {
                current_url = parsed.replace_ref(&latest);
                url_changed = true;
            }
        }

        if url_changed && let Some(range) = call.parsed.string_range("url") {
            updates.push(Update::new(
                format!("{}.url", call.kind.name()),
                format!("\"{}\"", current_url),
                range,
            ));
        }

        let strip_len: usize = call.parsed.ints.get("stripLen").copied().unwrap_or(0) as usize;

        let relative = call.parsed.strings.get("relative").cloned();
        let extra_prefix = call.parsed.strings.get("extraPrefix").cloned();
        let revert = call.parsed.bools.get("revert").copied().unwrap_or(false);

        let has_post_fetch = call
            .parsed
            .strings
            .get("postFetch")
            .is_some_and(|p| !p.is_empty());

        let decode = call
            .parsed
            .strings
            .get("decode")
            .cloned()
            .unwrap_or_else(|| "cat".to_string());
        let can_decode = decode == "cat";

        let needs_hash = (url_changed
            || call
                .parsed
                .strings
                .get("hash")
                .is_some_and(|h| h.is_empty())
            || call
                .parsed
                .strings
                .get("sha256")
                .is_some_and(|h| h.is_empty()))
            && !has_post_fetch
            && can_decode;

        if needs_hash {
            let has_hash_source =
                call.parsed.has_string("hash") || call.parsed.has_string("sha256");

            if has_hash_source {
                let options = crate::utils::PatchOptions {
                    strip_len,
                    relative,
                    extra_prefix,
                    excludes: call.parsed.pure_string_list("excludes").unwrap_or_default(),
                    includes: call.parsed.pure_string_list("includes").unwrap_or_default(),
                    hunks: call
                        .parsed
                        .list_ints
                        .get("hunks")
                        .map(|v| v.iter().map(|&i| i as usize).collect())
                        .unwrap_or_default(),
                    revert,
                };
                let result = crate::utils::PatchHasher::hash_patch_url(&current_url, &options);
                match result {
                    Ok(nar_hash) => {
                        if let Some(range) = call.parsed.string_range("hash") {
                            updates.push(Update::new(
                                format!("{}.hash", call.kind.name()),
                                format!("\"{}\"", nar_hash.sri),
                                range,
                            ));
                        }
                        if let Some(range) = call.parsed.string_range("sha256") {
                            updates.push(Update::new(
                                format!("{}.sha256", call.kind.name()),
                                format!("\"{}\"", nar_hash.nix32),
                                range,
                            ));
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning: could not prefetch hash for fetchpatch {}: {:#}",
                            current_url, e
                        );
                    }
                }
            }
        }

        if updates.is_empty() {
            Ok(None)
        } else {
            Ok(Some(updates))
        }
    }

    fn handle_branch_following(
        &self,
        call: &FetcherCall,
        git_url: &str,
        branch: &str,
        updates: &mut Vec<Update>,
    ) -> Result<Option<String>> {
        let new_sha = match GitFetcher::get_latest_commit(git_url, branch)? {
            Some(sha) => sha,
            None => {
                eprintln!(
                    "Warning: could not find branch '{}' for {}",
                    branch, git_url
                );
                return Ok(None);
            }
        };

        let current_ref = call
            .parsed
            .strings
            .get("rev")
            .or_else(|| call.parsed.strings.get("ref"));

        if let Some(current) = current_ref
            && current == &new_sha
        {
            return Ok(None);
        }

        let ref_key = if call.parsed.strings.contains_key("rev") {
            "rev"
        } else if call.kind == FetcherKind::BuiltinsFetchGit {
            "ref"
        } else {
            "rev"
        };

        if let Some(range) = call.parsed.string_range(ref_key) {
            updates.push(Update::new(
                format!("{}.rev", call.kind.name()),
                format!("\"{}\"", new_sha),
                range,
            ));

            Ok(Some(new_sha))
        } else {
            Ok(None)
        }
    }

    fn handle_version_update(
        &self,
        call: &FetcherCall,
        git_url: &str,
        updates: &mut Vec<Update>,
    ) -> Result<Option<String>> {
        let Some((version_key, current_version)) =
            version_ref_key_and_value(call.kind, &call.parsed)
        else {
            return Ok(None);
        };

        let latest = match GitFetcher::get_latest_tag_matching(git_url, Some(&current_version))? {
            Some(tag) => tag,
            None => return Ok(None),
        };

        if VersionDetector::compare(&current_version, &latest) != std::cmp::Ordering::Less {
            return Ok(None);
        }

        if let Some(range) = call.parsed.string_range(version_key) {
            updates.push(Update::new(
                format!("{}.{}", call.kind.name(), version_key),
                format!("\"{}\"", latest),
                range,
            ));

            Ok(Some(latest))
        } else {
            Ok(None)
        }
    }

    fn resolve_rev(call: &FetcherCall, git_url: &str) -> Option<String> {
        let key = preferred_ref_key(&call.parsed)?;
        let ref_value = call.parsed.strings.get(key)?;
        resolve_ref_for_prefetch(git_url, ref_value)
    }

    fn try_prefetch_hash(call: &FetcherCall, rev: &str, updates: &mut Vec<Update>) {
        if !call.parsed.has_string("hash") && !call.parsed.has_string("sha256") {
            return;
        }

        let result = Self::compute_hash(call, rev);

        match result {
            Ok(nar_hash) => {
                if let Some(range) = call.parsed.string_range("hash") {
                    updates.push(Update::new(
                        format!("{}.hash", call.kind.name()),
                        format!("\"{}\"", nar_hash.sri),
                        range,
                    ));
                }
                if let Some(range) = call.parsed.string_range("sha256") {
                    updates.push(Update::new(
                        format!("{}.sha256", call.kind.name()),
                        format!("\"{}\"", nar_hash.nix32),
                        range,
                    ));
                }
            }
            Err(e) => {
                let git_url = call.kind.git_url(&call.parsed).unwrap_or_default();
                eprintln!(
                    "Warning: could not prefetch hash for {} @ {}: {:#}",
                    git_url, rev, e
                );
            }
        }
    }

    fn try_prefetch_empty_hash(call: &FetcherCall, git_url: &str, updates: &mut Vec<Update>) {
        let has_empty_hash = call
            .parsed
            .strings
            .get("hash")
            .is_some_and(|h| h.is_empty())
            || call
                .parsed
                .strings
                .get("sha256")
                .is_some_and(|h| h.is_empty());

        if !has_empty_hash {
            return;
        }

        if let Some(rev) = Self::resolve_rev(call, git_url) {
            Self::try_prefetch_hash(call, &rev, updates);
        }
    }

    fn compute_hash(call: &FetcherCall, rev: &str) -> Result<NarHash> {
        let has_sparse_checkout = call
            .parsed
            .pure_string_list("sparseCheckout")
            .is_some_and(|v| !v.is_empty());
        match call.kind.hash_strategy(&call.parsed, has_sparse_checkout) {
            HashStrategy::Tarball => tarball::compute_hash(&call.kind, &call.parsed, rev),
            HashStrategy::Git => {
                let sparse_checkout = call
                    .parsed
                    .pure_string_list("sparseCheckout")
                    .unwrap_or_default();
                git_fetch::compute_hash(&call.kind, &call.parsed, rev, &sparse_checkout)
            }
            HashStrategy::Patch => {
                anyhow::bail!("Patch hashing should be handled via check_fetchpatch_call")
            }
            HashStrategy::None => anyhow::bail!("No hash needed for this fetcher"),
        }
    }

    fn is_src_of_active_mk_derivation(node: &NixNode) -> bool {
        let mut current = match node.parent() {
            Some(p) => p,
            None => return false,
        };

        while current.kind() == rnix::SyntaxKind::NODE_PAREN {
            current = match current.parent() {
                Some(p) => p,
                None => return false,
            };
        }

        if current.kind() != rnix::SyntaxKind::NODE_ATTRPATH_VALUE {
            return false;
        }
        let segments = current.attrpath_segments();
        if segments.len() != 1 || segments[0] != "src" {
            return false;
        }

        let attr_set = match current.parent() {
            Some(p) => p,
            None => return false,
        };
        if attr_set.kind() != rnix::SyntaxKind::NODE_ATTR_SET {
            return false;
        }

        let mut mk_derivation_apply = match attr_set.parent() {
            Some(p) => p,
            None => return false,
        };
        if mk_derivation_apply.kind() == rnix::SyntaxKind::NODE_LAMBDA {
            mk_derivation_apply = match mk_derivation_apply.parent() {
                Some(p) => p,
                None => return false,
            };
        }
        if mk_derivation_apply.kind() == rnix::SyntaxKind::NODE_PAREN {
            mk_derivation_apply = match mk_derivation_apply.parent() {
                Some(p) => p,
                None => return false,
            };
        }
        if mk_derivation_apply.kind() != rnix::SyntaxKind::NODE_APPLY {
            return false;
        }

        let func_name = match mk_derivation_apply.apply_function_name() {
            Some(name) => name,
            None => return false,
        };
        let short_name = func_name.rsplit('.').next().unwrap_or(&func_name);
        if short_name != "mkDerivation" {
            return false;
        }

        true
    }
}

impl UpdateRule for FetcherRule {
    fn name(&self) -> &str {
        "fetcher"
    }

    fn matches(&self, node: &NixNode) -> bool {
        if node.kind() != rnix::SyntaxKind::NODE_APPLY {
            return false;
        }
        if Self::is_src_of_active_mk_derivation(node) {
            return false;
        }
        true
    }

    fn check(&self, node: &NixNode) -> Result<Option<Vec<Update>>> {
        let call = match Self::try_extract_call(node) {
            Some(call) => call,
            None => return Ok(None),
        };

        match call.kind {
            FetcherKind::FetchPatch => Self::check_fetchpatch_call(&call),
            FetcherKind::BuiltinsFetchGit
            | FetcherKind::FetchGit
            | FetcherKind::FetchFromGitHub
            | FetcherKind::FetchFromGitLab
            | FetcherKind::FetchFromGitea
            | FetcherKind::FetchFromForgejo
            | FetcherKind::FetchFromCodeberg
            | FetcherKind::FetchFromBitbucket
            | FetcherKind::FetchFromSourcehut
            | FetcherKind::FetchFromGitiles
            | FetcherKind::FetchFromRepoOrCz => self.check_fetcher_call(&call),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::parser::NixFile;
    use crate::rules::traits::UpdateRule;

    fn parse_root(content: &str) -> crate::parser::NixNode {
        NixFile::parse(content).unwrap().root_node()
    }

    fn find_fetcher_apply(
        root: &crate::parser::NixNode,
        name: &str,
    ) -> Option<crate::parser::NixNode> {
        root.traverse().find(|node| {
            node.kind() == rnix::SyntaxKind::NODE_APPLY
                && node.apply_function_name().as_deref() == Some(name)
        })
    }

    #[test]
    fn test_is_src_of_mk_derivation_returns_true() {
        let content = r#"
stdenv.mkDerivation rec {
  name = "foo-${version}";
  version = "v1.0.0";
  src = fetchgit {
    url = "https://example.com/repo";
    rev = "0000000000000000000000000000000000000000";
    sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
  };
}
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        assert!(super::FetcherRule::is_src_of_active_mk_derivation(
            &fetcher_node
        ));
    }

    #[test]
    fn test_standalone_fetcher_returns_false() {
        let content = r#"
{
  src = fetchgit {
    url = "https://example.com/repo";
    rev = "v1.0.0";
    hash = "sha256-AAA=";
  };
}
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        assert!(!super::FetcherRule::is_src_of_active_mk_derivation(
            &fetcher_node
        ));
    }

    #[test]
    fn test_pkgs_dot_stdenv_dot_mk_derivation_returns_true() {
        let content = r#"
pkgs.stdenv.mkDerivation rec {
  name = "foo-${version}";
  version = "v1.0.0";
  src = fetchgit {
    url = "https://example.com/repo";
    rev = "0000000000000000000000000000000000000000";
    sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
  };
}
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        assert!(super::FetcherRule::is_src_of_active_mk_derivation(
            &fetcher_node
        ));
    }

    #[test]
    fn test_fetcher_non_src_attr_returns_false() {
        let content = r#"
stdenv.mkDerivation rec {
  name = "foo-${version}";
  version = "v1.0.0";
  patches = fetchgit {
    url = "https://example.com/repo";
    rev = "v1.0.0";
    hash = "sha256-AAA=";
  };
}
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        assert!(!super::FetcherRule::is_src_of_active_mk_derivation(
            &fetcher_node
        ));
    }

    #[test]
    fn test_fetcher_in_non_mk_derivation_returns_false() {
        let content = r#"
someOtherFunc rec {
  name = "foo-${version}";
  version = "v1.0.0";
  src = fetchgit {
    url = "https://example.com/repo";
    rev = "v1.0.0";
    hash = "sha256-AAA=";
  };
}
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        assert!(!super::FetcherRule::is_src_of_active_mk_derivation(
            &fetcher_node
        ));
    }

    #[test]
    fn test_matches_excludes_src_in_mk_derivation() {
        let content = r#"
stdenv.mkDerivation rec {
  name = "foo-${version}";
  version = "v1.0.0";
  src = fetchgit {
    url = "https://example.com/repo";
    rev = "0000000000000000000000000000000000000000";
    sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
  };
}
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        let rule = super::FetcherRule;
        assert!(!rule.matches(&fetcher_node));
    }

    #[test]
    fn test_matches_allows_standalone_fetcher() {
        let content = r#"
{
  src = fetchgit {
    url = "https://example.com/repo";
    rev = "v1.0.0";
    hash = "sha256-AAA=";
  };
}
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        let rule = super::FetcherRule;
        assert!(rule.matches(&fetcher_node));
    }

    #[test]
    fn test_matches_allows_patches_in_mk_derivation() {
        let content = r#"
stdenv.mkDerivation rec {
  name = "foo-${version}";
  version = "v1.0.0";
  patches = fetchgit {
    url = "https://example.com/repo";
    rev = "v1.0.0";
    hash = "sha256-AAA=";
  };
}
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        let rule = super::FetcherRule;
        assert!(rule.matches(&fetcher_node));
    }

    #[test]
    fn test_resolve_ref_for_prefetch_keeps_symbolic_ref() {
        let result = super::resolve_ref_for_prefetch("https://example.com/repo", "v1.2.3");
        assert_eq!(result.as_deref(), Some("v1.2.3"));
    }

    #[test]
    fn test_resolve_ref_for_prefetch_keeps_commit_hash() {
        let rev = "4f56fd184ef6020626492a6f954a486d54f8b7ba";
        let result = super::resolve_ref_for_prefetch("https://example.com/repo", rev);
        assert_eq!(result.as_deref(), Some(rev));
    }

    #[test]
    fn test_parse_fetcher_attrset_pure_strings() {
        let content = r#"{ url = "https://example.com"; rev = "v1.0"; fetchSubmodules = true; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let attrs = super::parse_fetcher_attrset(
            super::FetcherKind::FetchGit,
            &attr_set,
            &super::InterpolationSpec::none(),
        )
        .unwrap();
        assert_eq!(
            attrs.parsed.strings.get("url"),
            Some(&"https://example.com".to_string())
        );
        assert_eq!(attrs.parsed.strings.get("rev"), Some(&"v1.0".to_string()));
        assert_eq!(attrs.parsed.bools.get("fetchSubmodules"), Some(&true));
        assert!(attrs.interpolated.is_empty());
        assert!(attrs.interpolated_unresolved.is_empty());
        assert!(attrs.parsed.has_string("url"));
        assert!(attrs.parsed.has_string("rev"));
    }

    #[test]
    fn test_parse_fetcher_attrset_interpolated_unresolved_with_no_spec() {
        let content = r#"{ url = "https://example.com/${name}"; rev = "v1.0"; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let attrs = super::parse_fetcher_attrset(
            super::FetcherKind::FetchGit,
            &attr_set,
            &super::InterpolationSpec::none(),
        )
        .unwrap();
        assert!(!attrs.parsed.strings.contains_key("url"));
        assert_eq!(attrs.interpolated_unresolved, vec!["url"]);
        assert_eq!(attrs.parsed.strings.get("rev"), Some(&"v1.0".to_string()));
        assert!(attrs.interpolated.is_empty());
        assert!(attrs.parsed.has_string("url"));
    }

    #[test]
    fn test_parse_fetcher_attrset_interpolated_allowed_by_spec() {
        let content = r#"{ rev = "v${version}"; version = "1.0"; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let mut spec = super::InterpolationSpec::none();
        spec.allow(
            "rev",
            HashMap::from([("version".to_string(), "1.0".to_string())]),
        );
        let attrs =
            super::parse_fetcher_attrset(super::FetcherKind::FetchGit, &attr_set, &spec).unwrap();
        assert!(attrs.interpolated.contains_key("rev"));
        assert!(attrs.interpolated_unresolved.is_empty());
        assert!(!attrs.parsed.strings.contains_key("rev"));
        assert_eq!(
            attrs.parsed.strings.get("version"),
            Some(&"1.0".to_string())
        );
    }

    #[test]
    fn test_parse_fetcher_attrset_interpolated_not_matching_spec() {
        let content = r#"{ rev = "v${unknown}"; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let mut spec = super::InterpolationSpec::none();
        spec.allow(
            "rev",
            HashMap::from([("version".to_string(), "1.0".to_string())]),
        );
        let attrs =
            super::parse_fetcher_attrset(super::FetcherKind::FetchGit, &attr_set, &spec).unwrap();
        assert!(attrs.interpolated.is_empty());
        assert_eq!(attrs.interpolated_unresolved, vec!["rev"]);
    }

    #[test]
    fn test_parse_fetcher_attrset_dual_interpolation_vars() {
        let content = r#"{ rev = "v${version}"; owner = "test"; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let mut spec = super::InterpolationSpec::none();
        spec.allow(
            "rev",
            HashMap::from([
                ("version".to_string(), "1.0".to_string()),
                ("finalAttrs.version".to_string(), "1.0".to_string()),
            ]),
        );
        let attrs =
            super::parse_fetcher_attrset(super::FetcherKind::FetchGit, &attr_set, &spec).unwrap();
        assert!(attrs.interpolated.contains_key("rev"));
        assert!(attrs.interpolated_unresolved.is_empty());
        assert!(!attrs.parsed.strings.contains_key("rev"));
        assert_eq!(attrs.parsed.strings.get("owner"), Some(&"test".to_string()));
    }

    #[test]
    fn test_parse_fetcher_attrset_dual_interpolation_vars_dotted() {
        let content = r#"{ rev = "v${finalAttrs.version}"; owner = "test"; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let mut spec = super::InterpolationSpec::none();
        spec.allow(
            "rev",
            HashMap::from([
                ("version".to_string(), "1.0".to_string()),
                ("finalAttrs.version".to_string(), "1.0".to_string()),
            ]),
        );
        let attrs =
            super::parse_fetcher_attrset(super::FetcherKind::FetchGit, &attr_set, &spec).unwrap();
        assert!(attrs.interpolated.contains_key("rev"));
        assert!(attrs.interpolated_unresolved.is_empty());
        assert!(!attrs.parsed.strings.contains_key("rev"));
        assert_eq!(attrs.parsed.strings.get("owner"), Some(&"test".to_string()));
    }

    #[test]
    fn test_fetcher_skips_interpolated_operational_key() {
        let content = r#"
{
  src = fetchgit {
    url = "https://example.com/${name}";
    rev = "v1.0.0";
    hash = "sha256-AAA=";
  };
}
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        assert!(super::FetcherRule::try_extract_call(&fetcher_node).is_none());
    }

    #[test]
    fn test_fetcher_allows_unknown_interpolated_key() {
        let content = r#"
{
  src = fetchgit {
    url = "https://example.com/repo";
    rev = "v1.0.0";
    hash = "sha256-AAA=";
    name = "foo-${version}";
  };
}
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        let call = super::FetcherRule::try_extract_call(&fetcher_node);
        assert!(call.is_some());
        assert_eq!(
            call.unwrap().parsed.strings.get("rev"),
            Some(&"v1.0.0".to_string())
        );
    }

    #[test]
    fn test_is_src_of_lambda_wrapped_mk_derivation_returns_true() {
        let content = r#"
stdenv.mkDerivation (finalAttrs: {
  version = "v1.0.0";
  src = fetchgit {
    url = "https://example.com/repo";
    rev = "0000000000000000000000000000000000000000";
    sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
  };
})
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        assert!(super::FetcherRule::is_src_of_active_mk_derivation(
            &fetcher_node
        ));
    }

    #[test]
    fn test_matches_excludes_src_in_lambda_wrapped_mk_derivation() {
        let content = r#"
stdenv.mkDerivation (finalAttrs: {
  version = "v1.0.0";
  src = fetchgit {
    url = "https://example.com/repo";
    rev = "0000000000000000000000000000000000000000";
    sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
  };
})
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        let rule = super::FetcherRule;
        assert!(!rule.matches(&fetcher_node));
    }

    #[test]
    fn test_interpolation_spec_vars_for_field_merge() {
        let mut spec = super::InterpolationSpec::none();
        spec.allow_all(HashMap::from([("pname".to_string(), "foo".to_string())]));
        spec.allow(
            "rev",
            HashMap::from([("version".to_string(), "1.0".to_string())]),
        );
        let rev_vars = spec.vars_for_field("rev").unwrap();
        assert_eq!(rev_vars.get("pname"), Some(&"foo".to_string()));
        assert_eq!(rev_vars.get("version"), Some(&"1.0".to_string()));
        let owner_vars = spec.vars_for_field("owner").unwrap();
        assert_eq!(owner_vars.get("pname"), Some(&"foo".to_string()));
        assert!(!owner_vars.contains_key("version"));
        let unknown_vars = spec.vars_for_field("name").unwrap();
        assert_eq!(unknown_vars.get("pname"), Some(&"foo".to_string()));
    }

    #[test]
    fn test_interpolation_spec_vars_for_field_none() {
        let spec = super::InterpolationSpec::none();
        assert!(spec.vars_for_field("rev").is_none());
    }

    #[test]
    fn test_interpolation_spec_vars_for_field_only_allow_all() {
        let mut spec = super::InterpolationSpec::none();
        spec.allow_all(HashMap::from([("pname".to_string(), "foo".to_string())]));
        let vars = spec.vars_for_field("owner").unwrap();
        assert_eq!(vars.get("pname"), Some(&"foo".to_string()));
    }

    #[test]
    fn test_interpolation_spec_vars_for_field_only_field_specific() {
        let mut spec = super::InterpolationSpec::none();
        spec.allow(
            "rev",
            HashMap::from([("version".to_string(), "1.0".to_string())]),
        );
        let rev_vars = spec.vars_for_field("rev").unwrap();
        assert_eq!(rev_vars.get("version"), Some(&"1.0".to_string()));
        assert!(spec.vars_for_field("owner").is_none());
    }

    #[test]
    fn test_parse_fetcher_attrset_ident_resolution() {
        let content = r#"{ repo = pname; owner = "test-org"; rev = "v1.0.0"; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let mut spec = super::InterpolationSpec::none();
        spec.allow_idents(HashMap::from([("pname".to_string(), "my-pkg".to_string())]));
        let attrs =
            super::parse_fetcher_attrset(super::FetcherKind::FetchFromGitHub, &attr_set, &spec)
                .unwrap();
        assert_eq!(
            attrs.parsed.strings.get("repo"),
            Some(&"my-pkg".to_string())
        );
        assert_eq!(
            attrs.parsed.strings.get("owner"),
            Some(&"test-org".to_string())
        );
        assert_eq!(attrs.parsed.strings.get("rev"), Some(&"v1.0.0".to_string()));
    }

    #[test]
    fn test_parse_fetcher_attrset_ident_not_in_idents_returns_error() {
        let content = r#"{ repo = pname; owner = "test-org"; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let spec = super::InterpolationSpec::none();
        let result =
            super::parse_fetcher_attrset(super::FetcherKind::FetchFromGitHub, &attr_set, &spec);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_fetcher_attrset_allow_all_interpolation() {
        let content = r#"{ owner = "${pname}-org"; rev = "v1.0.0"; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let mut spec = super::InterpolationSpec::none();
        spec.allow_all(HashMap::from([("pname".to_string(), "foo".to_string())]));
        let attrs =
            super::parse_fetcher_attrset(super::FetcherKind::FetchFromGitHub, &attr_set, &spec)
                .unwrap();
        assert!(attrs.interpolated.contains_key("owner"));
        assert!(!attrs.interpolated_unresolved.iter().any(|k| k == "owner"));
        assert_eq!(attrs.parsed.strings.get("rev"), Some(&"v1.0.0".to_string()));
    }

    #[test]
    fn test_parse_fetcher_attrset_allow_all_and_field_specific_merge() {
        let content = r#"{ rev = "${pname}-${version}"; owner = "${pname}-org"; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let mut spec = super::InterpolationSpec::none();
        spec.allow_all(HashMap::from([("pname".to_string(), "foo".to_string())]));
        spec.allow(
            "rev",
            HashMap::from([("version".to_string(), "1.0".to_string())]),
        );
        let attrs =
            super::parse_fetcher_attrset(super::FetcherKind::FetchFromGitHub, &attr_set, &spec)
                .unwrap();
        assert!(attrs.interpolated.contains_key("rev"));
        assert!(attrs.interpolated.contains_key("owner"));
        assert!(attrs.interpolated_unresolved.is_empty());
    }
}
