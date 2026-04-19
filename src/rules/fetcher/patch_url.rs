//! URL pattern recognition for `fetchpatch` revision following.
//!
//! This module parses common patch URLs from hosting platforms
//! (GitHub, GitLab, Gitea/Forgejo/Codeberg, SourceHut, Bitbucket)
//! and extracts the current revision, enabling:
//!
//! - **Revision following** via `# follow:<branch>` comments — replace
//!   the commit SHA (or head ref of a compare range) with the latest SHA
//!   from the specified branch.
//! - **Version updates** — when the URL contains a version-like ref
//!   (e.g. a tag in a compare range), find and replace it with the
//!   latest matching version tag.

use crate::utils::VersionDetector;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// The hosting platform identified from a patch URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PatchPlatform {
    GitHub,
    GitLab,
    /// Also covers Forgejo, Codeberg, and any other Gitea-compatible host.
    Gitea,
    SourceHut,
    Bitbucket,
}

/// The type of patch reference extracted from a URL.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PatchRefType {
    /// A commit SHA (or tag) patch URL.
    ///
    /// - GitHub:  `/{project}/commit/{sha}[.patch|.diff]`
    /// - GitLab:  `/{project}/-/commit/{sha}[.patch|.diff]`
    /// - Gitea:   `/{project}/commit/{sha}[.patch|.diff]`
    /// - SourceHut: `/{project}/commit/{sha}[.patch|.mbox]`
    /// - Bitbucket: `/{project}/commits/{sha}`
    Commit { sha: String, suffix: String },
    /// A compare / diff range patch URL.
    ///
    /// - GitHub: `/{project}/compare/{base}...{head}[.patch|.diff]`
    /// - GitLab: `/{project}/-/compare/{base}...{head}[.patch|.diff]`
    /// - Gitea:  `/{project}/compare/{base}...{head}[.patch|.diff]`
    Compare {
        base: String,
        head: String,
        suffix: String,
    },
}

/// A parsed patch URL from a recognized hosting platform.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParsedPatchUrl {
    pub platform: PatchPlatform,
    /// Domain extracted from the URL (e.g. `github.com`, `gitlab.com`,
    /// `gitea.example.com`, `git.sr.ht`).
    pub domain: String,
    /// Full project path as it appears in the URL.
    ///
    /// - GitHub / Gitea / Bitbucket: `owner/repo`
    /// - GitLab: may include subgroups, e.g. `group/subgroup/project`
    /// - SourceHut: `~owner/repo` (with the `~` prefix)
    pub project: String,
    /// The type and content of the ref in the URL.
    pub ref_type: PatchRefType,
    /// Query string from the original URL (including leading `?`),
    /// or empty if none.
    pub query: String,
}

impl ParsedPatchUrl {
    /// Return the HTTPS git remote URL for this project,
    /// suitable for use with `git ls-remote`.
    pub fn git_remote_url(&self) -> String {
        format!("https://{}/{}", self.domain, self.project)
    }

    /// Return the current ref from the URL.
    ///
    /// For commit URLs, this is the SHA (or tag).
    /// For compare URLs, this is the head ref (the part after `...`).
    pub fn current_ref(&self) -> &str {
        match &self.ref_type {
            PatchRefType::Commit { sha, .. } => sha,
            PatchRefType::Compare { head, .. } => head,
        }
    }

    /// Check if the current ref looks like a version (for version updates).
    pub fn is_version_ref(&self) -> bool {
        VersionDetector::is_version(self.current_ref())
    }

    /// Construct a new patch URL with the ref replaced.
    ///
    /// For commit URLs, replaces the SHA.
    /// For compare URLs, replaces the head ref.
    /// Preserves any query parameters from the original URL.
    pub fn replace_ref(&self, new_ref: &str) -> String {
        let base_url = self.replace_ref_base(new_ref);
        if self.query.is_empty() {
            base_url
        } else {
            format!("{}{}", base_url, self.query)
        }
    }

    fn replace_ref_base(&self, new_ref: &str) -> String {
        match &self.ref_type {
            PatchRefType::Commit { suffix, .. } => match self.platform {
                PatchPlatform::GitHub | PatchPlatform::Gitea => format!(
                    "https://{}/{}/commit/{}{}",
                    self.domain, self.project, new_ref, suffix
                ),
                PatchPlatform::GitLab => format!(
                    "https://{}/{}/-/commit/{}{}",
                    self.domain, self.project, new_ref, suffix
                ),
                PatchPlatform::SourceHut => format!(
                    "https://{}/{}/commit/{}{}",
                    self.domain, self.project, new_ref, suffix
                ),
                PatchPlatform::Bitbucket => format!(
                    "https://{}/{}/commits/{}{}",
                    self.domain, self.project, new_ref, suffix
                ),
            },
            PatchRefType::Compare { base, suffix, .. } => match self.platform {
                PatchPlatform::GitHub | PatchPlatform::Gitea => format!(
                    "https://{}/{}/compare/{}...{}{}",
                    self.domain, self.project, base, new_ref, suffix
                ),
                PatchPlatform::GitLab => format!(
                    "https://{}/{}/-/compare/{}...{}{}",
                    self.domain, self.project, base, new_ref, suffix
                ),
                // SourceHut and Bitbucket compare URLs are less common
                // but follow the same pattern if encountered.
                PatchPlatform::SourceHut | PatchPlatform::Bitbucket => format!(
                    "https://{}/{}/compare/{}...{}{}",
                    self.domain, self.project, base, new_ref, suffix
                ),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Known file extensions for patch URLs across platforms.
const PATCH_EXTENSIONS: &[&str] = &[".patch", ".diff", ".mbox"];

/// Split a ref segment like `{sha}.patch` into `(sha, ".patch")`.
///
/// Strips a recognised extension if present.  If the segment has no
/// dot at all (i.e. no extension), the whole string is returned as the
/// ref with an empty suffix.  Returns `None` for an empty input or for
/// segments with an unrecognised extension (e.g. `.txt`).
fn split_ref_suffix(s: &str) -> Option<(String, String)> {
    if s.is_empty() {
        return None;
    }
    for ext in PATCH_EXTENSIONS {
        if let Some(base) = s.strip_suffix(ext)
            && !base.is_empty()
        {
            return Some((base.to_string(), ext.to_string()));
        }
    }
    // No recognised suffix — accept only if there is no dot, which
    // means the segment has no extension at all (e.g. a bare SHA or
    // tag).  Reject unrecognised extensions like `.txt`.
    if s.contains('.') {
        return None;
    }
    Some((s.to_string(), String::new()))
}

/// Split a compare ref segment like `{base}...{head}.patch` into
/// `(base, head, ".patch")`.
///
/// Only the three-dot separator (`...`) is supported.  The two-dot
/// separator (`..`) is ambiguous with dots in version numbers.
fn split_compare_ref(s: &str) -> Option<(String, String, String)> {
    let (ref_part, suffix) = split_ref_suffix(s)?;
    let (base, head) = ref_part.split_once("...")?;
    if !base.is_empty() && !head.is_empty() {
        Some((base.to_string(), head.to_string(), suffix))
    } else {
        None
    }
}

/// Parse a patch URL from a known hosting platform.
///
/// Returns `Some(ParsedPatchUrl)` if the URL matches a recognised
/// pattern, or `None` otherwise.
///
/// # Supported patterns
///
/// - **GitHub**: `https://github.com/{owner}/{repo}/commit/{sha}[.patch|.diff]`
///   and `https://github.com/{owner}/{repo}/compare/{base}...{head}[.patch|.diff]`
/// - **GitLab**: `https://{domain}/{project}/-/commit/{sha}[.patch|.diff]`
///   and `https://{domain}/{project}/-/compare/{base}...{head}[.patch|.diff]`
///   (`project` may include subgroups)
/// - **Gitea / Forgejo / Codeberg**:
///   `https://{domain}/{owner}/{repo}/commit/{sha}[.patch|.diff]`
///   and `https://{domain}/{owner}/{repo}/compare/{base}...{head}[.patch|.diff]`
/// - **SourceHut**: `https://{vc}.{sr.ht-domain}/~{owner}/{repo}/commit/{sha}[.patch|.mbox]`
/// - **Bitbucket**: `https://bitbucket.org/{owner}/{repo}/commits/{sha}`
pub(crate) fn parse_patch_url(url: &str) -> Option<ParsedPatchUrl> {
    let url = url.trim();

    // Must start with https://
    let rest = url.strip_prefix("https://")?;

    // Separate fragment (discard) and query string (preserve).
    let (before_fragment, _) = rest.split_once('#').unwrap_or((rest, ""));
    let (path_and_host, query_str) = before_fragment
        .split_once('?')
        .unwrap_or((before_fragment, ""));
    let query = if query_str.is_empty() {
        String::new()
    } else {
        format!("?{}", query_str)
    };

    // Split into domain and path.
    let (domain, path) = path_and_host.split_once('/')?;

    // --- Platform-specific dispatch ----------------------------------------

    // SourceHut: domains ending in .sr.ht with ~owner prefix in path.
    if domain.ends_with(".sr.ht") && path.starts_with('~') {
        return parse_simple_commit_compare_url(path, domain, PatchPlatform::SourceHut, query);
    }

    // GitLab: /-/commit/ or /-/compare/ anywhere in path.
    if path.contains("/-/commit/") || path.contains("/-/compare/") {
        return parse_gitlab_url(path, domain, query);
    }

    // Bitbucket: bitbucket.org domain
    if domain == "bitbucket.org" {
        return parse_bitbucket_url(path, domain, query);
    }

    // GitHub: github.com
    if domain == "github.com" {
        return parse_simple_commit_compare_url(path, domain, PatchPlatform::GitHub, query);
    }

    // Everything else with /commit/ or /compare/ → Gitea/Forgejo/Codeberg.
    parse_simple_commit_compare_url(path, domain, PatchPlatform::Gitea, query)
}

// ---------------------------------------------------------------------------
// Platform-specific parsers
// ---------------------------------------------------------------------------

/// Parse `/{project}/commit/{ref}[.ext]` or `/{project}/compare/{base}...{head}[.ext]`.
///
/// Used by GitHub, Gitea/Forgejo/Codeberg, and SourceHut (all share the
/// same path structure for commit/compare URLs).
fn parse_simple_commit_compare_url(
    path: &str,
    domain: &str,
    platform: PatchPlatform,
    query: String,
) -> Option<ParsedPatchUrl> {
    // Try commit pattern: {project}/commit/{ref}[.ext]
    if let Some(idx) = path.find("/commit/") {
        let project = &path[..idx];
        if project.is_empty() || !project.contains('/') {
            return None;
        }
        let ref_part = &path[idx + 8..]; // after "/commit/"
        let (sha, suffix) = split_ref_suffix(ref_part)?;
        return Some(ParsedPatchUrl {
            platform,
            domain: domain.to_string(),
            project: project.to_string(),
            ref_type: PatchRefType::Commit { sha, suffix },
            query,
        });
    }

    // Try compare pattern: {project}/compare/{base}...{head}[.ext]
    if let Some(idx) = path.find("/compare/") {
        let project = &path[..idx];
        if project.is_empty() || !project.contains('/') {
            return None;
        }
        let ref_part = &path[idx + 9..]; // after "/compare/"
        let (base, head, suffix) = split_compare_ref(ref_part)?;
        return Some(ParsedPatchUrl {
            platform,
            domain: domain.to_string(),
            project: project.to_string(),
            ref_type: PatchRefType::Compare { base, head, suffix },
            query,
        });
    }

    None
}

/// Parse GitLab-style `/{project}/-/commit/{ref}[.ext]` or
/// `/{project}/-/compare/{base}...{head}[.ext]`.
///
/// Works for gitlab.com and self-hosted GitLab instances.
fn parse_gitlab_url(path: &str, domain: &str, query: String) -> Option<ParsedPatchUrl> {
    // Try commit: {project}/-/commit/{ref}[.ext]
    if let Some(idx) = path.find("/-/commit/") {
        let project = &path[..idx];
        if project.is_empty() || !project.contains('/') {
            return None;
        }
        let ref_part = &path[idx + 10..]; // after "/-/commit/"
        let (sha, suffix) = split_ref_suffix(ref_part)?;
        return Some(ParsedPatchUrl {
            platform: PatchPlatform::GitLab,
            domain: domain.to_string(),
            project: project.to_string(),
            ref_type: PatchRefType::Commit { sha, suffix },
            query,
        });
    }

    // Try compare: {project}/-/compare/{base}...{head}[.ext]
    if let Some(idx) = path.find("/-/compare/") {
        let project = &path[..idx];
        if project.is_empty() || !project.contains('/') {
            return None;
        }
        let ref_part = &path[idx + 11..]; // after "/-/compare/"
        let (base, head, suffix) = split_compare_ref(ref_part)?;
        return Some(ParsedPatchUrl {
            platform: PatchPlatform::GitLab,
            domain: domain.to_string(),
            project: project.to_string(),
            ref_type: PatchRefType::Compare { base, head, suffix },
            query,
        });
    }

    None
}

/// Parse Bitbucket `/{owner}/{repo}/commits/{sha}` (no extension).
fn parse_bitbucket_url(path: &str, domain: &str, query: String) -> Option<ParsedPatchUrl> {
    let segments: Vec<&str> = path.split('/').collect();
    if segments.len() < 4 {
        return None;
    }

    let owner = segments[0];
    let repo = segments[1];
    let project = format!("{}/{}", owner, repo);

    if segments[2] != "commits" {
        return None;
    }

    // Bitbucket commit URLs don't use extensions.
    let sha = segments[3].to_string();
    if sha.is_empty() {
        return None;
    }

    Some(ParsedPatchUrl {
        platform: PatchPlatform::Bitbucket,
        domain: domain.to_string(),
        project,
        ref_type: PatchRefType::Commit {
            sha,
            suffix: String::new(),
        },
        query,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- GitHub commit -------------------------------------------------------

    #[test]
    fn test_parse_github_commit_patch() {
        let info =
            parse_patch_url("https://github.com/owner/repo/commit/abc123def456.patch").unwrap();
        assert_eq!(info.platform, PatchPlatform::GitHub);
        assert_eq!(info.domain, "github.com");
        assert_eq!(info.project, "owner/repo");
        assert_eq!(info.current_ref(), "abc123def456");
        assert_eq!(info.git_remote_url(), "https://github.com/owner/repo");
        assert_eq!(
            info.replace_ref("newsha789"),
            "https://github.com/owner/repo/commit/newsha789.patch"
        );
    }

    #[test]
    fn test_parse_github_commit_diff() {
        let info = parse_patch_url("https://github.com/owner/repo/commit/abc123.diff").unwrap();
        assert_eq!(info.current_ref(), "abc123");
        assert_eq!(
            info.replace_ref("newsha"),
            "https://github.com/owner/repo/commit/newsha.diff"
        );
    }

    #[test]
    fn test_parse_github_commit_full_sha() {
        let sha = "0f14e030b3a9391e761c03ce3c260730a78a4db6";
        let url = format!("https://github.com/owner/repo/commit/{}.patch", sha);
        let info = parse_patch_url(&url).unwrap();
        assert_eq!(info.current_ref(), sha);
    }

    #[test]
    fn test_parse_github_commit_tag_as_ref() {
        // GitHub allows using a tag name instead of a SHA in commit URLs.
        let info = parse_patch_url("https://github.com/owner/repo/commit/v1.0.0.patch").unwrap();
        assert_eq!(info.current_ref(), "v1.0.0");
    }

    // -- GitHub compare ------------------------------------------------------

    #[test]
    fn test_parse_github_compare_patch() {
        let info =
            parse_patch_url("https://github.com/owner/repo/compare/v1.0.0...v2.0.0.patch").unwrap();
        assert_eq!(info.platform, PatchPlatform::GitHub);
        assert_eq!(info.project, "owner/repo");
        match &info.ref_type {
            PatchRefType::Compare { base, head, suffix } => {
                assert_eq!(base, "v1.0.0");
                assert_eq!(head, "v2.0.0");
                assert_eq!(suffix, ".patch");
            }
            other => panic!("expected Compare, got {:?}", other),
        }
        assert_eq!(info.current_ref(), "v2.0.0");
        assert_eq!(
            info.replace_ref("v3.0.0"),
            "https://github.com/owner/repo/compare/v1.0.0...v3.0.0.patch"
        );
    }

    #[test]
    fn test_parse_github_compare_diff() {
        let info =
            parse_patch_url("https://github.com/owner/repo/compare/v1.0.0...v2.0.0.diff").unwrap();
        assert_eq!(info.current_ref(), "v2.0.0");
        assert_eq!(
            info.replace_ref("v3.0.0"),
            "https://github.com/owner/repo/compare/v1.0.0...v3.0.0.diff"
        );
    }

    #[test]
    fn test_parse_github_commit_with_query() {
        let info =
            parse_patch_url("https://github.com/owner/repo/commit/abc123.patch?full_index=1")
                .unwrap();
        assert_eq!(info.current_ref(), "abc123");
        assert_eq!(info.query, "?full_index=1");
        assert_eq!(
            info.replace_ref("newsha"),
            "https://github.com/owner/repo/commit/newsha.patch?full_index=1"
        );
    }

    #[test]
    fn test_parse_github_compare_with_query() {
        let info =
            parse_patch_url("https://github.com/owner/repo/compare/v1.0.0...v2.0.0.patch?w=1")
                .unwrap();
        assert_eq!(info.query, "?w=1");
        assert_eq!(
            info.replace_ref("v3.0.0"),
            "https://github.com/owner/repo/compare/v1.0.0...v3.0.0.patch?w=1"
        );
    }

    #[test]
    fn test_parse_github_commit_with_fragment() {
        // Fragment is stripped; query is preserved.
        let info =
            parse_patch_url("https://github.com/owner/repo/commit/abc123.patch#diff").unwrap();
        assert_eq!(info.current_ref(), "abc123");
        assert_eq!(info.query, "");
    }

    #[test]
    fn test_parse_github_commit_with_query_and_fragment() {
        let info =
            parse_patch_url("https://github.com/owner/repo/commit/abc123.patch?full_index=1#diff")
                .unwrap();
        assert_eq!(info.current_ref(), "abc123");
        assert_eq!(info.query, "?full_index=1");
    }

    // -- GitLab commit -------------------------------------------------------

    #[test]
    fn test_parse_gitlab_commit_patch() {
        let info = parse_patch_url("https://gitlab.com/owner/repo/-/commit/abc123.patch").unwrap();
        assert_eq!(info.platform, PatchPlatform::GitLab);
        assert_eq!(info.domain, "gitlab.com");
        assert_eq!(info.project, "owner/repo");
        assert_eq!(info.current_ref(), "abc123");
        assert_eq!(info.git_remote_url(), "https://gitlab.com/owner/repo");
    }

    #[test]
    fn test_parse_gitlab_compare_patch() {
        let info = parse_patch_url("https://gitlab.com/owner/repo/-/compare/v1.0.0...v2.0.0.diff")
            .unwrap();
        assert_eq!(info.current_ref(), "v2.0.0");
        assert_eq!(
            info.replace_ref("v3.0.0"),
            "https://gitlab.com/owner/repo/-/compare/v1.0.0...v3.0.0.diff"
        );
    }

    #[test]
    fn test_parse_gitlab_custom_domain() {
        let info =
            parse_patch_url("https://gitlab.example.com/group/project/-/commit/abc123.patch")
                .unwrap();
        assert_eq!(info.platform, PatchPlatform::GitLab);
        assert_eq!(info.domain, "gitlab.example.com");
        assert_eq!(info.project, "group/project");
    }

    #[test]
    fn test_parse_gitlab_nested_group() {
        let info =
            parse_patch_url("https://gitlab.com/group/subgroup/project/-/commit/abc123.patch")
                .unwrap();
        assert_eq!(info.project, "group/subgroup/project");
        assert_eq!(
            info.git_remote_url(),
            "https://gitlab.com/group/subgroup/project"
        );
    }

    #[test]
    fn test_parse_gitlab_compare_custom_domain() {
        let info =
            parse_patch_url("https://gitlab.corp.com/team/lib/-/compare/v1.0.0...v2.0.0.patch")
                .unwrap();
        assert_eq!(info.current_ref(), "v2.0.0");
        assert_eq!(
            info.replace_ref("v3.0.0"),
            "https://gitlab.corp.com/team/lib/-/compare/v1.0.0...v3.0.0.patch"
        );
    }

    // -- Codeberg / Gitea / Forgejo -----------------------------------------

    #[test]
    fn test_parse_codeberg_commit_patch() {
        let info = parse_patch_url("https://codeberg.org/owner/repo/commit/abc123.patch").unwrap();
        assert_eq!(info.platform, PatchPlatform::Gitea);
        assert_eq!(info.domain, "codeberg.org");
        assert_eq!(info.project, "owner/repo");
        assert_eq!(info.git_remote_url(), "https://codeberg.org/owner/repo");
    }

    #[test]
    fn test_parse_gitea_commit_patch() {
        let info =
            parse_patch_url("https://gitea.example.com/owner/repo/commit/abc123.patch").unwrap();
        assert_eq!(info.platform, PatchPlatform::Gitea);
        assert_eq!(info.domain, "gitea.example.com");
        assert_eq!(
            info.git_remote_url(),
            "https://gitea.example.com/owner/repo"
        );
    }

    #[test]
    fn test_parse_gitea_compare_patch() {
        let info =
            parse_patch_url("https://gitea.example.com/owner/repo/compare/v1.0.0...v2.0.0.patch")
                .unwrap();
        assert_eq!(info.current_ref(), "v2.0.0");
    }

    #[test]
    fn test_parse_forgejo_compare_diff() {
        let info =
            parse_patch_url("https://forgejo.dev/org/proj/compare/v0.1...v0.2.diff").unwrap();
        assert_eq!(info.current_ref(), "v0.2");
    }

    // -- SourceHut ------------------------------------------------------------

    #[test]
    fn test_parse_sourcehut_commit_patch() {
        let info = parse_patch_url("https://git.sr.ht/~owner/repo/commit/abc123.patch").unwrap();
        assert_eq!(info.platform, PatchPlatform::SourceHut);
        assert_eq!(info.domain, "git.sr.ht");
        assert_eq!(info.project, "~owner/repo");
        assert_eq!(info.current_ref(), "abc123");
        assert_eq!(info.git_remote_url(), "https://git.sr.ht/~owner/repo");
        assert_eq!(
            info.replace_ref("newsha"),
            "https://git.sr.ht/~owner/repo/commit/newsha.patch"
        );
    }

    #[test]
    fn test_parse_sourcehut_commit_mbox() {
        let info = parse_patch_url("https://git.sr.ht/~owner/repo/commit/abc123.mbox").unwrap();
        assert_eq!(info.current_ref(), "abc123");
        assert_eq!(
            info.replace_ref("newsha"),
            "https://git.sr.ht/~owner/repo/commit/newsha.mbox"
        );
    }

    #[test]
    fn test_parse_sourcehut_custom_domain() {
        let info =
            parse_patch_url("https://git.custom.sr.ht/~owner/repo/commit/abc123.patch").unwrap();
        assert_eq!(info.domain, "git.custom.sr.ht");
        assert_eq!(info.project, "~owner/repo");
    }

    // -- Bitbucket -----------------------------------------------------------

    #[test]
    fn test_parse_bitbucket_commit() {
        let info = parse_patch_url("https://bitbucket.org/owner/repo/commits/abc123").unwrap();
        assert_eq!(info.platform, PatchPlatform::Bitbucket);
        assert_eq!(info.domain, "bitbucket.org");
        assert_eq!(info.project, "owner/repo");
        assert_eq!(info.current_ref(), "abc123");
        assert_eq!(info.git_remote_url(), "https://bitbucket.org/owner/repo");
        assert_eq!(
            info.replace_ref("newsha"),
            "https://bitbucket.org/owner/repo/commits/newsha"
        );
    }

    #[test]
    fn test_parse_bitbucket_commit_full_sha() {
        let sha = "0f14e030b3a9391e761c03ce3c260730a78a4db6";
        let url = format!("https://bitbucket.org/owner/repo/commits/{}", sha);
        let info = parse_patch_url(&url).unwrap();
        assert_eq!(info.current_ref(), sha);
    }

    // -- Edge cases ----------------------------------------------------------

    #[test]
    fn test_parse_unknown_url_returns_none() {
        assert_eq!(parse_patch_url("https://example.com/some/path"), None);
    }

    #[test]
    fn test_parse_http_returns_none() {
        assert_eq!(
            parse_patch_url("http://github.com/owner/repo/commit/abc123.patch"),
            None
        );
    }

    #[test]
    fn test_parse_compare_no_ellipsis_returns_none() {
        // Single-ref compare URLs (no "...") are not supported.
        assert_eq!(
            parse_patch_url("https://github.com/owner/repo/compare/v1.0.0.patch"),
            None
        );
    }

    #[test]
    fn test_parse_empty_url_returns_none() {
        assert_eq!(parse_patch_url(""), None);
    }

    #[test]
    fn test_parse_no_domain_path_returns_none() {
        assert_eq!(parse_patch_url("https://github.com"), None);
    }

    #[test]
    fn test_parse_wrong_extension_returns_none() {
        assert_eq!(
            parse_patch_url("https://github.com/owner/repo/commit/abc123.txt"),
            None
        );
    }

    #[test]
    fn test_parse_github_no_extension() {
        // Without a recognised extension the entire segment is the ref.
        let info = parse_patch_url("https://github.com/owner/repo/commit/abc123").unwrap();
        assert_eq!(info.current_ref(), "abc123");
        assert_eq!(
            info.replace_ref("newsha"),
            "https://github.com/owner/repo/commit/newsha"
        );
    }

    #[test]
    fn test_is_version_ref() {
        let info =
            parse_patch_url("https://github.com/owner/repo/compare/v1.0.0...v2.0.0.patch").unwrap();
        assert!(info.is_version_ref());
    }

    #[test]
    fn test_parse_github_compare_with_sha_head() {
        let info =
            parse_patch_url("https://github.com/owner/repo/compare/v1.0.0...abc123def456.patch")
                .unwrap();
        // head is a SHA, not a version
        assert!(!info.is_version_ref());
    }
}
