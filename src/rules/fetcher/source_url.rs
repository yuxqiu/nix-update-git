//! URL pattern recognition for source revision following.
//!
//! This module parses common source URLs from hosting platforms
//! (GitHub, GitLab, Gitea/Forgejo/Codeberg, SourceHut, Bitbucket)
//! and extracts the current revision, enabling:
//!
//! - **Revision following** via `# follow:<branch>` comments — replace
//!   the commit SHA (or head ref of a compare range) with the latest SHA
//!   from the specified branch.
//! - **Version updates** — when the URL contains a version-like ref
//!   (e.g. a tag in a compare range or archive URL), find and replace it
//!   with the latest matching version tag.
//!
//! Two URL categories are supported:
//!
//! - **Patch URLs**: commit and compare URLs (`.patch`, `.diff`, `.mbox`).
//! - **Archive URLs**: tarball/zip download URLs (`/archive/{ref}.tar.gz`,
//!   `-/archive/{ref}/...`, `/get/{ref}.tar.gz`, `/+archive/{ref}.tar.gz`,
//!   `/snapshot/{ref}.tar.gz`, and `.zip` variants).

use crate::utils::VersionDetector;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// The hosting platform identified from a source URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourcePlatform {
    GitHub,
    GitLab,
    /// Also covers Forgejo, Codeberg, and any other Gitea-compatible host.
    Gitea,
    SourceHut,
    Bitbucket,
}

/// The type of source reference extracted from a URL.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SourceRefType {
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
    /// An archive / tarball download URL.
    ///
    /// - GitHub:  `/{project}/archive/{ref}.tar.gz` or
    ///   `/{project}/archive/refs/tags/{ref}.tar.gz`
    /// - GitLab:  `/{project}/-/archive/{ref}/{basename}.tar.gz`
    /// - Gitea/Forgejo/Codeberg: `/{project}/archive/{ref}.tar.gz`
    /// - SourceHut: `/{project}/archive/{ref}.tar.gz`
    /// - Bitbucket: `/{project}/get/{ref}.tar.gz`
    ///
    /// The `ref` in archive URLs is typically a version tag.
    Archive { tag_ref: String, suffix: String },
}

/// A parsed source URL from a recognized hosting platform.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParsedSourceUrl {
    pub platform: SourcePlatform,
    /// Domain extracted from the URL (e.g. `github.com`, `gitlab.com`,
    /// `gitea.example.com`, `git.sr.ht`).
    pub domain: String,
    /// Full project path as it appears in the URL.
    ///
    /// - GitHub / Gitea / Bitbucket: `owner/repo`
    /// - GitLab: may include subgroups, e.g. `group/subgroup/project`
    /// - SourceHut: `~owner/repo` (with the `~` prefix)
    pub project: String,
    /// For GitLab archive URLs, the repo name extracted from the basename
    /// segment (used for reconstruction). `None` for all other cases.
    pub repo_name: Option<String>,
    /// The type and content of the ref in the URL.
    pub ref_type: SourceRefType,
    /// Query string from the original URL (including leading `?`),
    /// or empty if none.
    pub query: String,
}

impl ParsedSourceUrl {
    /// Return the HTTPS git remote URL for this project,
    /// suitable for use with `git ls-remote`.
    pub fn git_remote_url(&self) -> String {
        format!("https://{}/{}", self.domain, self.project)
    }

    /// Return the current ref from the URL.
    ///
    /// For commit URLs, this is the SHA (or tag).
    /// For compare URLs, this is the head ref (the part after `...`).
    /// For archive URLs, this is the tag/ref.
    pub fn current_ref(&self) -> &str {
        match &self.ref_type {
            SourceRefType::Commit { sha, .. } => sha,
            SourceRefType::Compare { head, .. } => head,
            SourceRefType::Archive { tag_ref, .. } => tag_ref,
        }
    }

    /// Check if the current ref looks like a version (for version updates).
    pub fn is_version_ref(&self) -> bool {
        VersionDetector::is_version(self.current_ref())
    }

    /// Construct a new source URL with the ref replaced.
    ///
    /// For commit URLs, replaces the SHA.
    /// For compare URLs, replaces the head ref.
    /// For archive URLs, replaces the tag/ref.
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
            SourceRefType::Commit { suffix, .. } => match self.platform {
                SourcePlatform::GitHub | SourcePlatform::Gitea => format!(
                    "https://{}/{}/commit/{}{}",
                    self.domain, self.project, new_ref, suffix
                ),
                SourcePlatform::GitLab => format!(
                    "https://{}/{}/-/commit/{}{}",
                    self.domain, self.project, new_ref, suffix
                ),
                SourcePlatform::SourceHut => format!(
                    "https://{}/{}/commit/{}{}",
                    self.domain, self.project, new_ref, suffix
                ),
                SourcePlatform::Bitbucket => format!(
                    "https://{}/{}/commits/{}{}",
                    self.domain, self.project, new_ref, suffix
                ),
            },
            SourceRefType::Compare { base, suffix, .. } => match self.platform {
                SourcePlatform::GitHub | SourcePlatform::Gitea => format!(
                    "https://{}/{}/compare/{}...{}{}",
                    self.domain, self.project, base, new_ref, suffix
                ),
                SourcePlatform::GitLab => format!(
                    "https://{}/{}/-/compare/{}...{}{}",
                    self.domain, self.project, base, new_ref, suffix
                ),
                // SourceHut and Bitbucket compare URLs are less common
                // but follow the same pattern if encountered.
                SourcePlatform::SourceHut | SourcePlatform::Bitbucket => format!(
                    "https://{}/{}/compare/{}...{}{}",
                    self.domain, self.project, base, new_ref, suffix
                ),
            },
            SourceRefType::Archive { suffix, .. } => match self.platform {
                SourcePlatform::GitHub => format!(
                    "https://{}/{}/archive/{}{}",
                    self.domain, self.project, new_ref, suffix
                ),
                SourcePlatform::GitLab => {
                    // GitLab archive URLs include the repo name in the path:
                    // /-/archive/{ref}/{repo}-{ref}.tar.gz
                    let repo = self
                        .repo_name
                        .as_deref()
                        .unwrap_or_else(|| self.project.rsplit('/').next().unwrap_or(""));
                    format!(
                        "https://{}/{}/-/archive/{}/{}-{}{}",
                        self.domain, self.project, new_ref, repo, new_ref, suffix
                    )
                }
                SourcePlatform::Gitea => format!(
                    "https://{}/{}/archive/{}{}",
                    self.domain, self.project, new_ref, suffix
                ),
                SourcePlatform::SourceHut => format!(
                    "https://{}/{}/archive/{}{}",
                    self.domain, self.project, new_ref, suffix
                ),
                SourcePlatform::Bitbucket => format!(
                    "https://{}/{}/get/{}{}",
                    self.domain, self.project, new_ref, suffix
                ),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Known extensions
// ---------------------------------------------------------------------------

/// Known file extensions for patch URLs across platforms.
const PATCH_EXTENSIONS: &[&str] = &[".patch", ".diff", ".mbox"];

/// Known file extensions for archive/tarball URLs.
const ARCHIVE_EXTENSIONS: &[&str] = &[".tar.gz", ".tar.bz2", ".tar.xz", ".zip"];

// ---------------------------------------------------------------------------
// Splitting helpers
// ---------------------------------------------------------------------------

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

/// Split a ref segment like `{ref}.tar.gz` into `(ref, ".tar.gz")`.
///
/// Strips a recognised archive extension if present.  Returns `None`
/// for an empty input or for segments with an unrecognised extension.
/// If the segment has no dot at all, returns the whole thing with an
/// empty suffix (for cases like bare refs in `/get/{ref}`).
fn split_archive_suffix(s: &str) -> Option<(String, String)> {
    if s.is_empty() {
        return None;
    }
    for ext in ARCHIVE_EXTENSIONS {
        if let Some(base) = s.strip_suffix(ext)
            && !base.is_empty()
        {
            return Some((base.to_string(), ext.to_string()));
        }
    }
    // No recognised archive suffix — accept only if there is no dot.
    if s.contains('.') {
        return None;
    }
    Some((s.to_string(), String::new()))
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a source URL from a known hosting platform.
///
/// Returns `Some(ParsedSourceUrl)` if the URL matches a recognised
/// pattern, or `None` otherwise.
///
/// # Supported patterns
///
/// **Patch URLs:**
///
/// - **GitHub**: `/{owner}/{repo}/commit/{sha}[.patch|.diff]`
///   and `/{owner}/{repo}/compare/{base}...{head}[.patch|.diff]`
/// - **GitLab**: `/{project}/-/commit/{sha}[.patch|.diff]`
///   and `/{project}/-/compare/{base}...{head}[.patch|.diff]`
///   (`project` may include subgroups)
/// - **Gitea / Forgejo / Codeberg**:
///   `/{owner}/{repo}/commit/{sha}[.patch|.diff]`
///   and `/{owner}/{repo}/compare/{base}...{head}[.patch|.diff]`
/// - **SourceHut**: `/{~owner}/{repo}/commit/{sha}[.patch|.mbox]`
/// - **Bitbucket**: `/{owner}/{repo}/commits/{sha}`
///
/// **Archive URLs:**
///
/// - **GitHub**: `/{owner}/{repo}/archive/{ref}.tar.gz` or
///   `/{owner}/{repo}/archive/refs/tags/{ref}.tar.gz`
/// - **GitLab**: `/{project}/-/archive/{ref}/{basename}.tar.gz`
/// - **Gitea / Forgejo / Codeberg**: `/{owner}/{repo}/archive/{ref}.tar.gz`
/// - **SourceHut**: `/{~owner}/{repo}/archive/{ref}.tar.gz`
/// - **Bitbucket**: `/{owner}/{repo}/get/{ref}.tar.gz`
pub(crate) fn parse_source_url(url: &str) -> Option<ParsedSourceUrl> {
    let url = url.trim();

    // Must start with https:// or http://
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;

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
        if let Some(result) =
            parse_simple_archive_url(path, domain, SourcePlatform::SourceHut, &query)
        {
            return Some(result);
        }
        return parse_simple_commit_compare_url(path, domain, SourcePlatform::SourceHut, query);
    }

    // GitLab: /-/commit/ or /-/compare/ or /-/archive/ anywhere in path.
    if path.contains("/-/commit/") || path.contains("/-/compare/") || path.contains("/-/archive/") {
        if let Some(result) = parse_gitlab_archive_url(path, domain, &query) {
            return Some(result);
        }
        return parse_gitlab_url(path, domain, query);
    }

    // Bitbucket: bitbucket.org domain
    if domain == "bitbucket.org" {
        if let Some(result) = parse_bitbucket_archive_url(path, domain, &query) {
            return Some(result);
        }
        return parse_bitbucket_url(path, domain, query);
    }

    // GitHub: github.com
    if domain == "github.com" {
        if let Some(result) = parse_simple_archive_url(path, domain, SourcePlatform::GitHub, &query)
        {
            return Some(result);
        }
        return parse_simple_commit_compare_url(path, domain, SourcePlatform::GitHub, query);
    }

    // Everything else: try archive first, then commit/compare → Gitea/Forgejo/Codeberg.
    if let Some(result) = parse_simple_archive_url(path, domain, SourcePlatform::Gitea, &query) {
        return Some(result);
    }
    parse_simple_commit_compare_url(path, domain, SourcePlatform::Gitea, query)
}

// Backwards-compatible alias.
pub(crate) fn parse_patch_url(url: &str) -> Option<ParsedSourceUrl> {
    parse_source_url(url)
}

// ---------------------------------------------------------------------------
// Platform-specific parsers — patch/commit/compare URLs
// ---------------------------------------------------------------------------

/// Parse `/{project}/commit/{ref}[.ext]` or `/{project}/compare/{base}...{head}[.ext]`.
///
/// Used by GitHub, Gitea/Forgejo/Codeberg, and SourceHut (all share the
/// same path structure for commit/compare URLs).
fn parse_simple_commit_compare_url(
    path: &str,
    domain: &str,
    platform: SourcePlatform,
    query: String,
) -> Option<ParsedSourceUrl> {
    // Try commit pattern: {project}/commit/{ref}[.ext]
    if let Some(idx) = path.find("/commit/") {
        let project = &path[..idx];
        if project.is_empty() || !project.contains('/') {
            return None;
        }
        let ref_part = &path[idx + 8..]; // after "/commit/"
        let (sha, suffix) = split_ref_suffix(ref_part)?;
        return Some(ParsedSourceUrl {
            platform,
            domain: domain.to_string(),
            project: project.to_string(),
            repo_name: None,
            ref_type: SourceRefType::Commit { sha, suffix },
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
        return Some(ParsedSourceUrl {
            platform,
            domain: domain.to_string(),
            project: project.to_string(),
            repo_name: None,
            ref_type: SourceRefType::Compare { base, head, suffix },
            query,
        });
    }

    None
}

/// Parse GitLab-style `/{project}/-/commit/{ref}[.ext]` or
/// `/{project}/-/compare/{base}...{head}[.ext]`.
///
/// Works for gitlab.com and self-hosted GitLab instances.
fn parse_gitlab_url(path: &str, domain: &str, query: String) -> Option<ParsedSourceUrl> {
    // Try commit: {project}/-/commit/{ref}[.ext]
    if let Some(idx) = path.find("/-/commit/") {
        let project = &path[..idx];
        if project.is_empty() || !project.contains('/') {
            return None;
        }
        let ref_part = &path[idx + 10..]; // after "/-/commit/"
        let (sha, suffix) = split_ref_suffix(ref_part)?;
        return Some(ParsedSourceUrl {
            platform: SourcePlatform::GitLab,
            domain: domain.to_string(),
            project: project.to_string(),
            repo_name: None,
            ref_type: SourceRefType::Commit { sha, suffix },
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
        return Some(ParsedSourceUrl {
            platform: SourcePlatform::GitLab,
            domain: domain.to_string(),
            project: project.to_string(),
            repo_name: None,
            ref_type: SourceRefType::Compare { base, head, suffix },
            query,
        });
    }

    None
}

/// Parse Bitbucket `/{owner}/{repo}/commits/{sha}` (no extension).
fn parse_bitbucket_url(path: &str, domain: &str, query: String) -> Option<ParsedSourceUrl> {
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

    Some(ParsedSourceUrl {
        platform: SourcePlatform::Bitbucket,
        domain: domain.to_string(),
        project,
        repo_name: None,
        ref_type: SourceRefType::Commit {
            sha,
            suffix: String::new(),
        },
        query,
    })
}

// ---------------------------------------------------------------------------
// Platform-specific parsers — archive/tarball URLs
// ---------------------------------------------------------------------------

/// Parse `/{project}/archive/{ref}.tar.gz` or
/// `/{project}/archive/refs/tags/{ref}.tar.gz`.
///
/// Used by GitHub, Gitea/Forgejo/Codeberg, and SourceHut.
fn parse_simple_archive_url(
    path: &str,
    domain: &str,
    platform: SourcePlatform,
    query: &str,
) -> Option<ParsedSourceUrl> {
    // GitHub uses /archive/{ref}.tar.gz or /archive/refs/tags/{ref}.tar.gz
    // Gitea/Codeberg/Forgejo use /archive/{ref}.tar.gz
    // SourceHut uses /archive/{ref}.tar.gz
    if let Some(idx) = path.find("/archive/") {
        let project = &path[..idx];
        if project.is_empty() || !project.contains('/') {
            return None;
        }
        let archive_part = &path[idx + 9..]; // after "/archive/"

        // GitHub sometimes uses /archive/refs/tags/{ref}.tar.gz
        let ref_part = if let Some(tags_prefix) = archive_part.strip_prefix("refs/tags/") {
            tags_prefix
        } else {
            archive_part
        };

        let (tag_ref, suffix) = split_archive_suffix(ref_part)?;
        return Some(ParsedSourceUrl {
            platform,
            domain: domain.to_string(),
            project: project.to_string(),
            repo_name: None,
            ref_type: SourceRefType::Archive { tag_ref, suffix },
            query: query.to_string(),
        });
    }

    None
}

/// Parse GitLab archive URL:
/// `/{project}/-/archive/{ref}/{basename}.tar.gz`
fn parse_gitlab_archive_url(path: &str, domain: &str, query: &str) -> Option<ParsedSourceUrl> {
    if let Some(idx) = path.find("/-/archive/") {
        let project = &path[..idx];
        if project.is_empty() || !project.contains('/') {
            return None;
        }
        let after = &path[idx + 11..]; // after "/-/archive/"

        // Format: {ref}/{basename}.tar.gz
        // where basename is typically {repo}-{ref} or {ref}
        let (ref_and_rest, suffix) = split_archive_suffix(after)?;
        // ref_and_rest looks like: "{ref}/{basename}" where basename
        // can be anything — we only need the ref part.
        let (tag_ref, _basename) = ref_and_rest.split_once('/')?;

        Some(ParsedSourceUrl {
            platform: SourcePlatform::GitLab,
            domain: domain.to_string(),
            project: project.to_string(),
            // Extract repo name from the project path for URL reconstruction.
            repo_name: Some(project.rsplit('/').next().unwrap_or(project).to_string()),
            ref_type: SourceRefType::Archive {
                tag_ref: tag_ref.to_string(),
                suffix,
            },
            query: query.to_string(),
        })
    } else {
        None
    }
}

/// Parse Bitbucket archive URL:
/// `/{owner}/{repo}/get/{ref}.tar.gz` or
/// `/{owner}/{repo}/get/refs/tags/{ref}.tar.gz`
fn parse_bitbucket_archive_url(path: &str, domain: &str, query: &str) -> Option<ParsedSourceUrl> {
    let segments: Vec<&str> = path.split('/').collect();
    if segments.len() < 4 || segments[2] != "get" {
        return None;
    }

    let owner = segments[0];
    let repo = segments[1];
    let project = format!("{}/{}", owner, repo);

    // Collect everything after "get/" as a single path segment
    let ref_part = &path[path.find("/get/").unwrap() + 5..];

    // Bitbucket may use /get/refs/tags/{ref}.tar.gz
    let ref_part = if let Some(tags_prefix) = ref_part.strip_prefix("refs/tags/") {
        tags_prefix
    } else {
        ref_part
    };

    let (tag_ref, suffix) = split_archive_suffix(ref_part)?;

    Some(ParsedSourceUrl {
        platform: SourcePlatform::Bitbucket,
        domain: domain.to_string(),
        project,
        repo_name: None,
        ref_type: SourceRefType::Archive { tag_ref, suffix },
        query: query.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- GitHub commit (patch) ---------------------------------------------------

    #[test]
    fn test_parse_github_commit_patch() {
        let info =
            parse_source_url("https://github.com/owner/repo/commit/abc123def456.patch").unwrap();
        assert_eq!(info.platform, SourcePlatform::GitHub);
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
        let info = parse_source_url("https://github.com/owner/repo/commit/abc123.diff").unwrap();
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
        let info = parse_source_url(&url).unwrap();
        assert_eq!(info.current_ref(), sha);
    }

    #[test]
    fn test_parse_github_commit_tag_as_ref() {
        let info = parse_source_url("https://github.com/owner/repo/commit/v1.0.0.patch").unwrap();
        assert_eq!(info.current_ref(), "v1.0.0");
    }

    // -- GitHub compare (patch) -------------------------------------------------

    #[test]
    fn test_parse_github_compare_patch() {
        let info = parse_source_url("https://github.com/owner/repo/compare/v1.0.0...v2.0.0.patch")
            .unwrap();
        assert_eq!(info.platform, SourcePlatform::GitHub);
        assert_eq!(info.project, "owner/repo");
        match &info.ref_type {
            SourceRefType::Compare { base, head, suffix } => {
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
            parse_source_url("https://github.com/owner/repo/compare/v1.0.0...v2.0.0.diff").unwrap();
        assert_eq!(info.current_ref(), "v2.0.0");
        assert_eq!(
            info.replace_ref("v3.0.0"),
            "https://github.com/owner/repo/compare/v1.0.0...v3.0.0.diff"
        );
    }

    #[test]
    fn test_parse_github_commit_with_query() {
        let info =
            parse_source_url("https://github.com/owner/repo/commit/abc123.patch?full_index=1")
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
            parse_source_url("https://github.com/owner/repo/compare/v1.0.0...v2.0.0.patch?w=1")
                .unwrap();
        assert_eq!(info.query, "?w=1");
        assert_eq!(
            info.replace_ref("v3.0.0"),
            "https://github.com/owner/repo/compare/v1.0.0...v3.0.0.patch?w=1"
        );
    }

    #[test]
    fn test_parse_github_commit_with_fragment() {
        let info =
            parse_source_url("https://github.com/owner/repo/commit/abc123.patch#diff").unwrap();
        assert_eq!(info.current_ref(), "abc123");
        assert_eq!(info.query, "");
    }

    #[test]
    fn test_parse_github_commit_with_query_and_fragment() {
        let info =
            parse_source_url("https://github.com/owner/repo/commit/abc123.patch?full_index=1#diff")
                .unwrap();
        assert_eq!(info.current_ref(), "abc123");
        assert_eq!(info.query, "?full_index=1");
    }

    // -- GitHub archive (tarball) -----------------------------------------------

    #[test]
    fn test_parse_github_archive_tarball() {
        let info = parse_source_url("https://github.com/owner/repo/archive/v1.0.0.tar.gz").unwrap();
        assert_eq!(info.platform, SourcePlatform::GitHub);
        assert_eq!(info.project, "owner/repo");
        match &info.ref_type {
            SourceRefType::Archive { tag_ref, suffix } => {
                assert_eq!(tag_ref, "v1.0.0");
                assert_eq!(suffix, ".tar.gz");
            }
            other => panic!("expected Archive, got {:?}", other),
        }
        assert_eq!(info.current_ref(), "v1.0.0");
        assert!(info.is_version_ref());
        assert_eq!(
            info.replace_ref("v2.0.0"),
            "https://github.com/owner/repo/archive/v2.0.0.tar.gz"
        );
    }

    #[test]
    fn test_parse_github_archive_refs_tags() {
        let info =
            parse_source_url("https://github.com/owner/repo/archive/refs/tags/v1.0.0.tar.gz")
                .unwrap();
        assert_eq!(info.platform, SourcePlatform::GitHub);
        match &info.ref_type {
            SourceRefType::Archive { tag_ref, suffix } => {
                assert_eq!(tag_ref, "v1.0.0");
                assert_eq!(suffix, ".tar.gz");
            }
            other => panic!("expected Archive, got {:?}", other),
        }
        // Replace should produce the shorter form without refs/tags
        assert_eq!(
            info.replace_ref("v2.0.0"),
            "https://github.com/owner/repo/archive/v2.0.0.tar.gz"
        );
    }

    #[test]
    fn test_parse_github_archive_zip() {
        let info = parse_source_url("https://github.com/owner/repo/archive/v1.0.0.zip").unwrap();
        match &info.ref_type {
            SourceRefType::Archive { tag_ref, suffix } => {
                assert_eq!(tag_ref, "v1.0.0");
                assert_eq!(suffix, ".zip");
            }
            other => panic!("expected Archive, got {:?}", other),
        }
        assert_eq!(
            info.replace_ref("v2.0.0"),
            "https://github.com/owner/repo/archive/v2.0.0.zip"
        );
    }

    // -- GitLab commit (patch) ---------------------------------------------------

    #[test]
    fn test_parse_gitlab_commit_patch() {
        let info = parse_source_url("https://gitlab.com/owner/repo/-/commit/abc123.patch").unwrap();
        assert_eq!(info.platform, SourcePlatform::GitLab);
        assert_eq!(info.domain, "gitlab.com");
        assert_eq!(info.project, "owner/repo");
        assert_eq!(info.current_ref(), "abc123");
        assert_eq!(info.git_remote_url(), "https://gitlab.com/owner/repo");
    }

    #[test]
    fn test_parse_gitlab_compare_patch() {
        let info = parse_source_url("https://gitlab.com/owner/repo/-/compare/v1.0.0...v2.0.0.diff")
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
            parse_source_url("https://gitlab.example.com/group/project/-/commit/abc123.patch")
                .unwrap();
        assert_eq!(info.platform, SourcePlatform::GitLab);
        assert_eq!(info.domain, "gitlab.example.com");
        assert_eq!(info.project, "group/project");
    }

    #[test]
    fn test_parse_gitlab_nested_group() {
        let info =
            parse_source_url("https://gitlab.com/group/subgroup/project/-/commit/abc123.patch")
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
            parse_source_url("https://gitlab.corp.com/team/lib/-/compare/v1.0.0...v2.0.0.patch")
                .unwrap();
        assert_eq!(info.current_ref(), "v2.0.0");
        assert_eq!(
            info.replace_ref("v3.0.0"),
            "https://gitlab.corp.com/team/lib/-/compare/v1.0.0...v3.0.0.patch"
        );
    }

    // -- GitLab archive (tarball) -----------------------------------------------

    #[test]
    fn test_parse_gitlab_archive_tarball() {
        let info =
            parse_source_url("https://gitlab.com/owner/repo/-/archive/v1.0.0/repo-v1.0.0.tar.gz")
                .unwrap();
        assert_eq!(info.platform, SourcePlatform::GitLab);
        assert_eq!(info.project, "owner/repo");
        match &info.ref_type {
            SourceRefType::Archive { tag_ref, suffix } => {
                assert_eq!(tag_ref, "v1.0.0");
                assert_eq!(suffix, ".tar.gz");
            }
            other => panic!("expected Archive, got {:?}", other),
        }
        assert_eq!(info.current_ref(), "v1.0.0");
        assert_eq!(
            info.replace_ref("v2.0.0"),
            "https://gitlab.com/owner/repo/-/archive/v2.0.0/repo-v2.0.0.tar.gz"
        );
    }

    #[test]
    fn test_parse_gitlab_archive_zip() {
        let info =
            parse_source_url("https://gitlab.com/owner/repo/-/archive/v1.0.0/repo-v1.0.0.zip")
                .unwrap();
        match &info.ref_type {
            SourceRefType::Archive { tag_ref, suffix } => {
                assert_eq!(tag_ref, "v1.0.0");
                assert_eq!(suffix, ".zip");
            }
            other => panic!("expected Archive, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_gitlab_archive_nested_group() {
        let info = parse_source_url(
            "https://gitlab.com/group/subgroup/proj/-/archive/v1.0.0/proj-v1.0.0.tar.gz",
        )
        .unwrap();
        assert_eq!(info.project, "group/subgroup/proj");
        assert_eq!(info.repo_name, Some("proj".to_string()));
        assert_eq!(
            info.replace_ref("v2.0.0"),
            "https://gitlab.com/group/subgroup/proj/-/archive/v2.0.0/proj-v2.0.0.tar.gz"
        );
    }

    // -- Codeberg / Gitea / Forgejo (patch) --------------------------------------

    #[test]
    fn test_parse_codeberg_commit_patch() {
        let info = parse_source_url("https://codeberg.org/owner/repo/commit/abc123.patch").unwrap();
        assert_eq!(info.platform, SourcePlatform::Gitea);
        assert_eq!(info.domain, "codeberg.org");
        assert_eq!(info.project, "owner/repo");
        assert_eq!(info.git_remote_url(), "https://codeberg.org/owner/repo");
    }

    #[test]
    fn test_parse_gitea_commit_patch() {
        let info =
            parse_source_url("https://gitea.example.com/owner/repo/commit/abc123.patch").unwrap();
        assert_eq!(info.platform, SourcePlatform::Gitea);
        assert_eq!(info.domain, "gitea.example.com");
        assert_eq!(
            info.git_remote_url(),
            "https://gitea.example.com/owner/repo"
        );
    }

    #[test]
    fn test_parse_gitea_compare_patch() {
        let info =
            parse_source_url("https://gitea.example.com/owner/repo/compare/v1.0.0...v2.0.0.patch")
                .unwrap();
        assert_eq!(info.current_ref(), "v2.0.0");
    }

    #[test]
    fn test_parse_forgejo_compare_diff() {
        let info =
            parse_source_url("https://forgejo.dev/org/proj/compare/v0.1...v0.2.diff").unwrap();
        assert_eq!(info.current_ref(), "v0.2");
    }

    // -- Gitea/Codeberg archive (tarball) ---------------------------------------

    #[test]
    fn test_parse_gitea_archive_tarball() {
        let info =
            parse_source_url("https://gitea.example.com/owner/repo/archive/v1.0.0.tar.gz").unwrap();
        assert_eq!(info.platform, SourcePlatform::Gitea);
        assert_eq!(info.project, "owner/repo");
        match &info.ref_type {
            SourceRefType::Archive { tag_ref, suffix } => {
                assert_eq!(tag_ref, "v1.0.0");
                assert_eq!(suffix, ".tar.gz");
            }
            other => panic!("expected Archive, got {:?}", other),
        }
        assert_eq!(
            info.replace_ref("v2.0.0"),
            "https://gitea.example.com/owner/repo/archive/v2.0.0.tar.gz"
        );
    }

    #[test]
    fn test_parse_codeberg_archive_tarball() {
        let info =
            parse_source_url("https://codeberg.org/owner/repo/archive/v1.0.0.tar.gz").unwrap();
        assert_eq!(info.platform, SourcePlatform::Gitea);
        assert_eq!(
            info.replace_ref("v2.0.0"),
            "https://codeberg.org/owner/repo/archive/v2.0.0.tar.gz"
        );
    }

    // -- SourceHut (patch) -------------------------------------------------------

    #[test]
    fn test_parse_sourcehut_commit_patch() {
        let info = parse_source_url("https://git.sr.ht/~owner/repo/commit/abc123.patch").unwrap();
        assert_eq!(info.platform, SourcePlatform::SourceHut);
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
        let info = parse_source_url("https://git.sr.ht/~owner/repo/commit/abc123.mbox").unwrap();
        assert_eq!(info.current_ref(), "abc123");
        assert_eq!(
            info.replace_ref("newsha"),
            "https://git.sr.ht/~owner/repo/commit/newsha.mbox"
        );
    }

    #[test]
    fn test_parse_sourcehut_custom_domain() {
        let info =
            parse_source_url("https://git.custom.sr.ht/~owner/repo/commit/abc123.patch").unwrap();
        assert_eq!(info.domain, "git.custom.sr.ht");
        assert_eq!(info.project, "~owner/repo");
    }

    // -- SourceHut archive (tarball) ---------------------------------------------

    #[test]
    fn test_parse_sourcehut_archive_tarball() {
        let info = parse_source_url("https://git.sr.ht/~owner/repo/archive/v1.0.0.tar.gz").unwrap();
        assert_eq!(info.platform, SourcePlatform::SourceHut);
        assert_eq!(info.project, "~owner/repo");
        match &info.ref_type {
            SourceRefType::Archive { tag_ref, suffix } => {
                assert_eq!(tag_ref, "v1.0.0");
                assert_eq!(suffix, ".tar.gz");
            }
            other => panic!("expected Archive, got {:?}", other),
        }
        assert_eq!(
            info.replace_ref("v2.0.0"),
            "https://git.sr.ht/~owner/repo/archive/v2.0.0.tar.gz"
        );
    }

    // -- Bitbucket (patch) ------------------------------------------------------

    #[test]
    fn test_parse_bitbucket_commit() {
        let info = parse_source_url("https://bitbucket.org/owner/repo/commits/abc123").unwrap();
        assert_eq!(info.platform, SourcePlatform::Bitbucket);
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
        let info = parse_source_url(&url).unwrap();
        assert_eq!(info.current_ref(), sha);
    }

    // -- Bitbucket archive (tarball) ---------------------------------------------

    #[test]
    fn test_parse_bitbucket_archive_tarball() {
        let info = parse_source_url("https://bitbucket.org/owner/repo/get/v1.0.0.tar.gz").unwrap();
        assert_eq!(info.platform, SourcePlatform::Bitbucket);
        assert_eq!(info.project, "owner/repo");
        match &info.ref_type {
            SourceRefType::Archive { tag_ref, suffix } => {
                assert_eq!(tag_ref, "v1.0.0");
                assert_eq!(suffix, ".tar.gz");
            }
            other => panic!("expected Archive, got {:?}", other),
        }
        assert_eq!(
            info.replace_ref("v2.0.0"),
            "https://bitbucket.org/owner/repo/get/v2.0.0.tar.gz"
        );
    }

    #[test]
    fn test_parse_bitbucket_archive_refs_tags() {
        let info = parse_source_url("https://bitbucket.org/owner/repo/get/refs/tags/v1.0.0.tar.gz")
            .unwrap();
        match &info.ref_type {
            SourceRefType::Archive { tag_ref, suffix } => {
                assert_eq!(tag_ref, "v1.0.0");
                assert_eq!(suffix, ".tar.gz");
            }
            other => panic!("expected Archive, got {:?}", other),
        }
        // Replace should produce the shorter form without refs/tags
        assert_eq!(
            info.replace_ref("v2.0.0"),
            "https://bitbucket.org/owner/repo/get/v2.0.0.tar.gz"
        );
    }

    // -- Edge cases --------------------------------------------------------------

    #[test]
    fn test_parse_unknown_url_returns_none() {
        assert_eq!(parse_source_url("https://example.com/some/path"), None);
    }

    #[test]
    fn test_parse_http_url() {
        let result = parse_source_url("http://github.com/owner/repo/commit/abc123.patch").unwrap();
        assert_eq!(result.domain, "github.com");
        assert_eq!(result.project, "owner/repo");
    }

    #[test]
    fn test_parse_compare_no_ellipsis_returns_none() {
        assert_eq!(
            parse_source_url("https://github.com/owner/repo/compare/v1.0.0.patch"),
            None
        );
    }

    #[test]
    fn test_parse_empty_url_returns_none() {
        assert_eq!(parse_source_url(""), None);
    }

    #[test]
    fn test_parse_no_domain_path_returns_none() {
        assert_eq!(parse_source_url("https://github.com"), None);
    }

    #[test]
    fn test_parse_wrong_extension_returns_none() {
        assert_eq!(
            parse_source_url("https://github.com/owner/repo/commit/abc123.txt"),
            None
        );
    }

    #[test]
    fn test_parse_github_no_extension() {
        let info = parse_source_url("https://github.com/owner/repo/commit/abc123").unwrap();
        assert_eq!(info.current_ref(), "abc123");
        assert_eq!(
            info.replace_ref("newsha"),
            "https://github.com/owner/repo/commit/newsha"
        );
    }

    #[test]
    fn test_is_version_ref() {
        let info = parse_source_url("https://github.com/owner/repo/compare/v1.0.0...v2.0.0.patch")
            .unwrap();
        assert!(info.is_version_ref());
    }

    #[test]
    fn test_parse_github_compare_with_sha_head() {
        let info =
            parse_source_url("https://github.com/owner/repo/compare/v1.0.0...abc123def456.patch")
                .unwrap();
        // head is a SHA, not a version
        assert!(!info.is_version_ref());
    }

    #[test]
    fn test_archive_version_ref() {
        let info = parse_source_url("https://github.com/owner/repo/archive/v1.0.0.tar.gz").unwrap();
        assert!(info.is_version_ref());
    }

    #[test]
    fn test_archive_sha_ref_not_version() {
        let info =
            parse_source_url("https://github.com/owner/repo/archive/abc123def456.tar.gz").unwrap();
        assert!(!info.is_version_ref());
    }

    // -- Backward-compatible alias -----------------------------------------------

    #[test]
    fn test_parse_patch_url_alias() {
        let info = parse_patch_url("https://github.com/owner/repo/commit/abc123.patch").unwrap();
        assert_eq!(info.platform, SourcePlatform::GitHub);
        assert_eq!(info.current_ref(), "abc123");
    }
}
