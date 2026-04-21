use std::collections::HashMap;

use anyhow::Result;

use crate::parser::{NixNode, TextRange};
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
    params: &HashMap<String, String>,
) -> Option<(&'static str, String)> {
    if let Some(tag) = params.get("tag") {
        return Some(("tag", tag.clone()));
    }
    if let Some(rev) = params.get("rev") {
        if is_commit_hash(rev) || !VersionDetector::is_version(rev) {
            return None;
        }
        return Some(("rev", rev.clone()));
    }
    if let Some(ref_val) = params.get("ref")
        && kind == FetcherKind::BuiltinsFetchGit
    {
        if is_commit_hash(ref_val) || !VersionDetector::is_version(ref_val) {
            return None;
        }
        return Some(("ref", ref_val.clone()));
    }
    None
}

pub(crate) fn preferred_ref_key(params: &HashMap<String, String>) -> Option<&'static str> {
    if params.contains_key("tag") {
        Some("tag")
    } else if params.contains_key("rev") {
        Some("rev")
    } else if params.contains_key("ref") {
        Some("ref")
    } else {
        None
    }
}

/// Resolve a ref value to a revision suitable for prefetching.
///
/// Currently returns the ref as-is for non-empty values (commit hashes
/// and symbolic refs like tags are passed through unchanged). The
/// `git_url` parameter is reserved for future use where symbolic refs
/// may be resolved to commit SHAs via `git ls-remote`.
pub(crate) fn resolve_ref_for_prefetch(_git_url: &str, ref_value: &str) -> Option<String> {
    if ref_value.is_empty() {
        return None;
    }
    Some(ref_value.to_string())
}
/// Keys that affect version resolution, URL construction, or hash
/// computation in a fetcher call. If any of these are interpolated
/// and not permitted by the `InterpolationSpec`, we conservatively
/// skip the call.
pub(crate) const OPERATIONAL_KEYS: &[&str] = &[
    "url",
    "owner",
    "repo",
    "domain",
    "githubBase",
    "vc",
    "tag",
    "rev",
    "ref",
    "hash",
    "sha256",
    "fetchSubmodules",
    "submodules",
    "deepClone",
    "leaveDotGit",
    "fetchLFS",
    "branchName",
    "rootDir",
    "stripLen",
    "relative",
    "extraPrefix",
    "excludes",
    "includes",
    "hunks",
    "revert",
    "decode",
    "postFetch",
    "nativeBuildInputs",
];

/// Specifies which fetcher attribute fields are allowed to contain
/// string interpolation, and what variable bindings are available
/// for resolution verification.
pub(crate) struct InterpolationSpec {
    /// Map from field name to variable bindings.
    /// E.g., `{"rev": {"version": "1.0.0"}}` means the `rev` field
    /// may use `${version}` interpolation.
    allowed: HashMap<String, HashMap<String, String>>,
    /// When set, any field may use these variable bindings for
    /// interpolation. Field-specific `allowed` entries take
    /// precedence (their variables are merged on top of
    /// `allow_all_vars`).
    allow_all_vars: Option<HashMap<String, String>>,
    /// Ident-name → string-value bindings for resolving bare ident
    /// values. E.g., `{"pname": "foo"}` resolves `repo = pname;`
    /// into `repo = "foo"`.
    ident_vars: HashMap<String, String>,
}

impl InterpolationSpec {
    /// Create a spec that does not allow any interpolated fields.
    pub(crate) fn none() -> Self {
        Self {
            allowed: HashMap::new(),
            allow_all_vars: None,
            ident_vars: HashMap::new(),
        }
    }

    /// Allow a specific field to be interpolated with the given
    /// variable bindings. The parser will verify that the
    /// interpolation can be fully resolved using these variables
    /// before accepting the field.
    pub(crate) fn allow(&mut self, field: &str, vars: HashMap<String, String>) {
        self.allowed.insert(field.to_string(), vars);
    }

    /// Allow interpolation in **any** field using the given variable
    /// bindings. Field-specific `allow()` entries are merged on top
    /// of these defaults, so their variables supplement or override
    /// the catch-all set.
    pub(crate) fn allow_all(&mut self, vars: HashMap<String, String>) {
        self.allow_all_vars = Some(vars);
    }

    /// Register ident bindings for bare ident resolution.
    /// When a fetcher attribute value is a bare identifier (e.g.
    /// `repo = pname`), the parser will look it up here and, if
    /// found, treat it as a pure string value.
    pub(crate) fn allow_idents(&mut self, idents: HashMap<String, String>) {
        self.ident_vars = idents;
    }

    /// Look up the effective variable bindings for a field.
    /// Field-specific entries are merged on top of `allow_all_vars`,
    /// so callers get the union of both.
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

/// Result of parsing a fetcher attrset.
pub(crate) struct FetcherAttrs {
    /// Pure (non-interpolated) string values and boolean ident values.
    pub params: HashMap<String, String>,
    /// Byte ranges of all string-valued attributes (both pure and
    /// interpolated).
    pub source_ranges: HashMap<String, TextRange>,
    /// Interpolated attributes that matched the `InterpolationSpec`
    /// (field is allowed and the interpolation can be resolved with
    /// the given variables). The template node is stored for callers
    /// that need to extract affixes or re-resolve with different
    /// variable values.
    pub interpolated: HashMap<String, NixNode>,
    /// Interpolated attributes that did NOT match the
    /// `InterpolationSpec` (either the field is not allowed, or the
    /// interpolation couldn't be resolved with the given variables).
    /// Only the key names are stored so callers can detect
    /// unsupported interpolations in operational fields.
    pub interpolated_unresolved: Vec<String>,
    /// Items from the `sparseCheckout` list attribute.
    pub sparse_checkout: Vec<String>,
    /// Items from the `excludes` list attribute.
    pub excludes: Vec<String>,
    /// Items from the `includes` list attribute.
    pub includes: Vec<String>,
    /// Items from the `hunks` list attribute (1-based indices).
    pub hunks: Vec<usize>,
}

/// Parse the attribute set of a fetcher call into structured fields.
///
/// The `InterpolationSpec` controls which fields may contain string
/// interpolation. Fields whose interpolation matches the spec are
/// stored in `FetcherAttrs::interpolated` with their template nodes.
/// All other interpolated fields are recorded in
/// `FetcherAttrs::interpolated_unresolved` so callers can detect
/// unsupported interpolations (e.g., in operational keys).
pub(crate) fn parse_fetcher_attrset(attr_set: &NixNode, spec: &InterpolationSpec) -> FetcherAttrs {
    let mut params = HashMap::new();
    let mut source_ranges = HashMap::new();
    let mut interpolated = HashMap::new();
    let mut interpolated_unresolved = Vec::new();
    let mut sparse_checkout = Vec::new();
    let mut excludes = Vec::new();
    let mut includes = Vec::new();
    let mut hunks = Vec::new();

    for child in attr_set.children() {
        if child.kind() != rnix::SyntaxKind::NODE_ATTRPATH_VALUE {
            continue;
        }
        let segments = child.attrpath_segments();
        if segments.len() != 1 {
            continue;
        }
        let key = segments[0].clone();

        if let Some(value) = child.attr_value() {
            if value.kind() == rnix::SyntaxKind::NODE_STRING {
                let range = value.text_range();
                source_ranges.insert(key.clone(), range);

                if let Some(content) = value.pure_string_content() {
                    params.insert(key, content);
                } else if let Some(vars) = spec.vars_for_field(&key) {
                    if value.interpolated_string_content(&vars).is_some() {
                        interpolated.insert(key, value);
                    } else {
                        interpolated_unresolved.push(key);
                    }
                } else {
                    interpolated_unresolved.push(key);
                }
            } else if value.kind() == rnix::SyntaxKind::NODE_IDENT {
                let trimmed = value.text_trimmed();
                if trimmed == "true" || trimmed == "false" {
                    params.insert(key, trimmed);
                } else if let Some(resolved) = spec.ident_vars.get(&trimmed) {
                    // Resolve bare ident references (e.g. `repo = pname`)
                    // using the ident bindings from the spec.
                    params.insert(key, resolved.clone());
                }
            } else if value.kind() == rnix::SyntaxKind::NODE_LITERAL {
                // Handle integer/float literals (e.g. `stripLen = 1`)
                // Store the text representation so numeric attributes are
                // available in params for lookups like stripLen parsing.
                let range = value.text_range();
                source_ranges.insert(key.clone(), range);
                params.insert(key, value.text_trimmed().to_string());
            } else if key == "sparseCheckout" && value.kind() == rnix::SyntaxKind::NODE_LIST {
                for item in value.children() {
                    if item.kind() == rnix::SyntaxKind::NODE_STRING
                        && let Some(content) = item.pure_string_content()
                    {
                        sparse_checkout.push(content);
                    }
                }
            } else if key == "excludes" && value.kind() == rnix::SyntaxKind::NODE_LIST {
                for item in value.children() {
                    if item.kind() == rnix::SyntaxKind::NODE_STRING
                        && let Some(content) = item.pure_string_content()
                    {
                        excludes.push(content);
                    }
                }
            } else if key == "includes" && value.kind() == rnix::SyntaxKind::NODE_LIST {
                for item in value.children() {
                    if item.kind() == rnix::SyntaxKind::NODE_STRING
                        && let Some(content) = item.pure_string_content()
                    {
                        includes.push(content);
                    }
                }
            } else if key == "hunks" && value.kind() == rnix::SyntaxKind::NODE_LIST {
                for item in value.children() {
                    if item.kind() == rnix::SyntaxKind::NODE_LITERAL
                        && let Ok(num) = item.text_trimmed().parse::<usize>()
                    {
                        hunks.push(num);
                    }
                }
            }
        }
    }

    FetcherAttrs {
        params,
        source_ranges,
        interpolated,
        interpolated_unresolved,
        sparse_checkout,
        excludes,
        includes,
        hunks,
    }
}

struct FetcherCall {
    kind: FetcherKind,
    params: HashMap<String, String>,
    source_ranges: HashMap<String, TextRange>,
    pinned: bool,
    follow_branch: Option<String>,
    sparse_checkout: Vec<String>,
    excludes: Vec<String>,
    includes: Vec<String>,
    hunks: Vec<usize>,
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

        let attrs = parse_fetcher_attrset(&arg, &InterpolationSpec::none());

        // Conservatively skip if any operational key is interpolated
        // but not permitted by the spec (which is empty here).
        if attrs
            .interpolated_unresolved
            .iter()
            .any(|k| OPERATIONAL_KEYS.contains(&k.as_str()))
        {
            return None;
        }

        let pinned = arg.has_pin_comment() || node.has_pin_comment();
        let follow_branch = arg
            .follow_branch_comment()
            .or_else(|| node.follow_branch_comment());

        Some(FetcherCall {
            kind,
            params: attrs.params,
            source_ranges: attrs.source_ranges,
            pinned,
            follow_branch,
            sparse_checkout: attrs.sparse_checkout,
            excludes: attrs.excludes,
            includes: attrs.includes,
            hunks: attrs.hunks,
        })
    }

    fn check_fetcher_call(&self, call: &FetcherCall) -> Result<Option<Vec<Update>>> {
        let git_url = match call.kind.git_url(&call.params) {
            Some(url) => url,
            None => return Ok(None),
        };

        let mut updates = Vec::new();
        let mut version_updated_rev: Option<String> = None;

        // Case 1: not pinned -> check version update
        if !call.pinned {
            if let Some(branch) = &call.follow_branch {
                version_updated_rev =
                    self.handle_branch_following(call, &git_url, branch, &mut updates)?;
            } else {
                version_updated_rev = self.handle_version_update(call, &git_url, &mut updates)?;
            }
        }

        // Case 2: update hash if needed
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

    /// Handle a fetchpatch call: revision following, version updates,
    /// and empty-hash filling.
    ///
    /// Three cases are handled:
    ///
    /// 1. **`# follow:<branch>`** — when the fetchpatch has a follow
    ///    comment, parse the URL to identify the hosting platform and
    ///    extract the current ref, then query `git ls-remote` for the
    ///    latest commit on the specified branch.  The ref in the URL is
    ///    replaced with the new SHA.
    ///
    /// 2. **Version update (not pinned)** — when the URL contains a
    ///    version-like ref (e.g. the head of a compare range like
    ///    `v2.0.0`), check for a newer matching tag and update the URL.
    ///
    /// 3. **Empty hash filling** — when `hash` or `sha256` is an empty
    ///    string, compute and fill the patch hash.  This also runs
    ///    whenever the URL changes (cases 1–2) to keep the hash in sync.
    fn check_fetchpatch_call(call: &FetcherCall) -> Result<Option<Vec<Update>>> {
        let url = match call.params.get("url") {
            Some(url) => url,
            None => return Ok(None),
        };

        let mut updates = Vec::new();
        let mut current_url = url.clone();
        let mut url_changed = false;

        // Try to parse the URL for revision following / version updates.
        let parsed_url = patch_url::parse_patch_url(url);

        // Case 1: # follow:<branch> — revision following
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
        } else if !call.pinned {
            // Case 2: not pinned — try version update for URLs with
            // version-like refs (typically compare ranges).
            if let Some(parsed) = &parsed_url
                && parsed.is_version_ref()
            {
                let git_url = parsed.git_remote_url();
                let current = parsed.current_ref();
                if let Ok(Some(latest)) =
                    GitFetcher::get_latest_tag_matching(&git_url, Some(current))
                    && VersionDetector::compare(current, &latest) == std::cmp::Ordering::Less
                {
                    current_url = parsed.replace_ref(&latest);
                    url_changed = true;
                }
            }
        }

        // If URL changed, emit an update for it.
        if url_changed && let Some(range) = call.source_ranges.get("url") {
            updates.push(Update::new(
                format!("{}.url", call.kind.name()),
                format!("\"{}\"", current_url),
                *range,
            ));
        }

        // Determine strip count: fetchpatch passes `--strip=${stripLen}`
        // to filterdiff, which strips that many path components from the
        // output. The subsequent `filterdiff -p1` only affects matching
        // (—strip-match), NOT the output paths. So the total strip is
        // just stripLen (default 0).
        let strip_len: usize = call
            .params
            .get("stripLen")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let relative = call.params.get("relative").cloned();
        let extra_prefix = call.params.get("extraPrefix").cloned();
        let revert = call.params.get("revert") == Some(&"true".to_string());

        // If postFetch is set (non-empty), we can't compute the hash
        // because it's arbitrary shell code that may modify the output.
        let has_post_fetch = call.params.get("postFetch").is_some_and(|p| !p.is_empty());

        // If decode is not "cat" (or absent), we can't compute the hash
        // unless we natively support the decode command. Currently we
        // only support the default ("cat" / absent).
        let decode = call
            .params
            .get("decode")
            .cloned()
            .unwrap_or_else(|| "cat".to_string());
        let can_decode = decode == "cat";

        // Compute / recompute hash when:
        // 1. URL changed (need to re-hash the new URL content), or
        // 2. hash / sha256 is empty (fill it in).
        let needs_hash = (url_changed
            || call.params.get("hash").is_some_and(|h| h.is_empty())
            || call.params.get("sha256").is_some_and(|h| h.is_empty()))
            && !has_post_fetch
            && can_decode;

        if needs_hash {
            let has_hash_source = call.source_ranges.contains_key("hash")
                || call.source_ranges.contains_key("sha256");

            if has_hash_source {
                let options = crate::utils::PatchOptions {
                    strip_len,
                    relative,
                    extra_prefix,
                    excludes: call.excludes.clone(),
                    includes: call.includes.clone(),
                    hunks: call.hunks.clone(),
                    revert,
                };
                let result = crate::utils::PatchHasher::hash_patch_url(&current_url, &options);
                match result {
                    Ok(nar_hash) => {
                        if let Some(range) = call.source_ranges.get("hash") {
                            updates.push(Update::new(
                                format!("{}.hash", call.kind.name()),
                                format!("\"{}\"", nar_hash.sri),
                                *range,
                            ));
                        }
                        if let Some(range) = call.source_ranges.get("sha256") {
                            updates.push(Update::new(
                                format!("{}.sha256", call.kind.name()),
                                format!("\"{}\"", nar_hash.nix32),
                                *range,
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

        let current_ref = call.params.get("rev").or_else(|| call.params.get("ref"));

        if let Some(current) = current_ref
            && current == &new_sha
        {
            return Ok(None);
        }

        let ref_key = if call.params.contains_key("rev") {
            "rev"
        } else if call.kind == FetcherKind::BuiltinsFetchGit {
            "ref"
        } else {
            "rev"
        };

        if let Some(range) = call.source_ranges.get(ref_key) {
            updates.push(Update::new(
                format!("{}.rev", call.kind.name()),
                format!("\"{}\"", new_sha),
                *range,
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
            version_ref_key_and_value(call.kind, &call.params)
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

        if let Some(range) = call.source_ranges.get(version_key) {
            updates.push(Update::new(
                format!("{}.{}", call.kind.name(), version_key),
                format!("\"{}\"", latest),
                *range,
            ));

            Ok(Some(latest))
        } else {
            Ok(None)
        }
    }

    fn resolve_rev(call: &FetcherCall, git_url: &str) -> Option<String> {
        let key = preferred_ref_key(&call.params)?;
        let ref_value = call.params.get(key)?;
        resolve_ref_for_prefetch(git_url, ref_value)
    }

    fn try_prefetch_hash(call: &FetcherCall, rev: &str, updates: &mut Vec<Update>) {
        if !call.source_ranges.contains_key("hash") && !call.source_ranges.contains_key("sha256") {
            return;
        }

        let result = Self::compute_hash(call, rev);

        match result {
            Ok(nar_hash) => {
                if let Some(range) = call.source_ranges.get("hash") {
                    updates.push(Update::new(
                        format!("{}.hash", call.kind.name()),
                        format!("\"{}\"", nar_hash.sri),
                        *range,
                    ));
                }
                if let Some(range) = call.source_ranges.get("sha256") {
                    updates.push(Update::new(
                        format!("{}.sha256", call.kind.name()),
                        format!("\"{}\"", nar_hash.nix32),
                        *range,
                    ));
                }
            }
            Err(e) => {
                let git_url = call.kind.git_url(&call.params).unwrap_or_default();
                eprintln!(
                    "Warning: could not prefetch hash for {} @ {}: {:#}",
                    git_url, rev, e
                );
            }
        }
    }

    fn try_prefetch_empty_hash(call: &FetcherCall, git_url: &str, updates: &mut Vec<Update>) {
        let has_empty_hash = call.params.get("hash").is_some_and(|h| h.is_empty())
            || call.params.get("sha256").is_some_and(|h| h.is_empty());

        if !has_empty_hash {
            return;
        }

        if let Some(rev) = Self::resolve_rev(call, git_url) {
            Self::try_prefetch_hash(call, &rev, updates);
        }
    }

    fn compute_hash(call: &FetcherCall, rev: &str) -> Result<NarHash> {
        let has_sparse_checkout = !call.sparse_checkout.is_empty();
        match call.kind.hash_strategy(&call.params, has_sparse_checkout) {
            HashStrategy::Tarball => tarball::compute_hash(&call.kind, &call.params, rev),
            HashStrategy::Git => {
                git_fetch::compute_hash(&call.kind, &call.params, rev, &call.sparse_checkout)
            }
            HashStrategy::Patch => {
                // Patch hashing is handled separately in check_fetchpatch_call
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

        // Walk up from the attrset through lambda/paren layers to find
        // the mkDerivation call. Handles both:
        //   mkDerivation { src = fetch ...; }     (direct attrset)
        //   mkDerivation (finalAttrs: { src = fetch ...; })  (lambda-wrapped)
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

        // fetchpatch uses a URL-based hash strategy (flat SHA-256 of the
        // normalized patch content), not a git-based one.
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
        let attrs = super::parse_fetcher_attrset(&attr_set, &super::InterpolationSpec::none());
        assert_eq!(
            attrs.params.get("url"),
            Some(&"https://example.com".to_string())
        );
        assert_eq!(attrs.params.get("rev"), Some(&"v1.0".to_string()));
        assert_eq!(
            attrs.params.get("fetchSubmodules"),
            Some(&"true".to_string())
        );
        assert!(attrs.interpolated.is_empty());
        assert!(attrs.interpolated_unresolved.is_empty());
        assert!(attrs.source_ranges.contains_key("url"));
        assert!(attrs.source_ranges.contains_key("rev"));
    }

    #[test]
    fn test_parse_fetcher_attrset_interpolated_unresolved_with_no_spec() {
        let content = r#"{ url = "https://example.com/${name}"; rev = "v1.0"; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let attrs = super::parse_fetcher_attrset(&attr_set, &super::InterpolationSpec::none());
        // url is interpolated but not allowed by spec → unresolved
        assert!(!attrs.params.contains_key("url"));
        assert_eq!(attrs.interpolated_unresolved, vec!["url"]);
        // rev is pure → parsed normally
        assert_eq!(attrs.params.get("rev"), Some(&"v1.0".to_string()));
        assert!(attrs.interpolated.is_empty());
        // Range still recorded for url (it's a string node)
        assert!(attrs.source_ranges.contains_key("url"));
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
        let attrs = super::parse_fetcher_attrset(&attr_set, &spec);
        // rev is interpolated and allowed → goes to interpolated
        assert!(attrs.interpolated.contains_key("rev"));
        assert!(attrs.interpolated_unresolved.is_empty());
        assert!(!attrs.params.contains_key("rev"));
        // version is pure → parsed normally
        assert_eq!(attrs.params.get("version"), Some(&"1.0".to_string()));
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
        let attrs = super::parse_fetcher_attrset(&attr_set, &spec);
        // rev uses ${unknown} but spec only provides version → unresolved
        assert!(attrs.interpolated.is_empty());
        assert_eq!(attrs.interpolated_unresolved, vec!["rev"]);
    }

    #[test]
    fn test_parse_fetcher_attrset_dual_interpolation_vars() {
        // When both "version" and "finalAttrs.version" are allowed,
        // rev = "v${version}" should resolve via the "version" binding.
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
        let attrs = super::parse_fetcher_attrset(&attr_set, &spec);
        assert!(attrs.interpolated.contains_key("rev"));
        assert!(attrs.interpolated_unresolved.is_empty());
        assert!(!attrs.params.contains_key("rev"));
        assert_eq!(attrs.params.get("owner"), Some(&"test".to_string()));
    }

    #[test]
    fn test_parse_fetcher_attrset_dual_interpolation_vars_dotted() {
        // When both "version" and "finalAttrs.version" are allowed,
        // rev = "v${finalAttrs.version}" should resolve via the
        // "finalAttrs.version" binding.
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
        let attrs = super::parse_fetcher_attrset(&attr_set, &spec);
        assert!(attrs.interpolated.contains_key("rev"));
        assert!(attrs.interpolated_unresolved.is_empty());
        assert!(!attrs.params.contains_key("rev"));
        assert_eq!(attrs.params.get("owner"), Some(&"test".to_string()));
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
        // try_extract_call should return None because url (an
        // operational key) is interpolated but not permitted.
        assert!(super::FetcherRule::try_extract_call(&fetcher_node).is_none());
    }

    #[test]
    fn test_fetcher_allows_non_operational_interpolated_key() {
        // A non-operational key like "name" being interpolated should
        // not cause the fetcher to be skipped.
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
        // "name" is not in OPERATIONAL_KEYS, so the call should still
        // be extracted (rev is pure).
        let call = super::FetcherRule::try_extract_call(&fetcher_node);
        assert!(call.is_some());
        assert_eq!(call.unwrap().params.get("rev"), Some(&"v1.0.0".to_string()));
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
        // rev merges allow_all + field-specific
        let rev_vars = spec.vars_for_field("rev").unwrap();
        assert_eq!(rev_vars.get("pname"), Some(&"foo".to_string()));
        assert_eq!(rev_vars.get("version"), Some(&"1.0".to_string()));
        // owner only gets allow_all
        let owner_vars = spec.vars_for_field("owner").unwrap();
        assert_eq!(owner_vars.get("pname"), Some(&"foo".to_string()));
        assert!(!owner_vars.contains_key("version"));
        // unknown field gets allow_all
        let name_vars = spec.vars_for_field("name").unwrap();
        assert_eq!(name_vars.get("pname"), Some(&"foo".to_string()));
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
        let attrs = super::parse_fetcher_attrset(&attr_set, &spec);
        // bare ident pname should be resolved via ident_vars
        assert_eq!(attrs.params.get("repo"), Some(&"my-pkg".to_string()));
        assert_eq!(attrs.params.get("owner"), Some(&"test-org".to_string()));
        assert_eq!(attrs.params.get("rev"), Some(&"v1.0.0".to_string()));
    }

    #[test]
    fn test_parse_fetcher_attrset_ident_not_in_idents_ignored() {
        let content = r#"{ repo = pname; owner = "test-org"; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        // No ident_vars configured — pname is not resolved
        let spec = super::InterpolationSpec::none();
        let attrs = super::parse_fetcher_attrset(&attr_set, &spec);
        assert!(!attrs.params.contains_key("repo"));
        assert_eq!(attrs.params.get("owner"), Some(&"test-org".to_string()));
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
        let attrs = super::parse_fetcher_attrset(&attr_set, &spec);
        // owner interpolation is allowed via allow_all
        assert!(attrs.interpolated.contains_key("owner"));
        assert!(!attrs.interpolated_unresolved.iter().any(|k| k == "owner"));
        assert_eq!(attrs.params.get("rev"), Some(&"v1.0.0".to_string()));
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
        let attrs = super::parse_fetcher_attrset(&attr_set, &spec);
        // rev uses both pname (from allow_all) and version (from field-specific)
        assert!(attrs.interpolated.contains_key("rev"));
        // owner uses only pname (from allow_all)
        assert!(attrs.interpolated.contains_key("owner"));
        assert!(attrs.interpolated_unresolved.is_empty());
    }
}
