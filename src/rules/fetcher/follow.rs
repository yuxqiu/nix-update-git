//! `# follow:` directive parsing and resolution.

use crate::rules::traits::CheckWarning;
use crate::utils::{GitFetcher, VersionDetector};

pub(super) enum FollowSpec {
    Branch(String),
    Regex(regex::Regex),
    Semver(semver::VersionReq),
}

pub(super) fn parse_follow_spec(s: &str) -> (Option<FollowSpec>, Vec<CheckWarning>) {
    let (kind, rest) = match s.split_once(' ') {
        Some((k, r)) => (k, r.trim()),
        None => return (None, vec![]),
    };
    match kind {
        "branch" => {
            if rest.is_empty() {
                return (None, vec![]);
            }
            (Some(FollowSpec::Branch(rest.to_string())), vec![])
        }
        "regex" => {
            if rest.is_empty() {
                return (None, vec![]);
            }
            match regex::Regex::new(&format!("^(?:{})$", rest)) {
                Ok(re) => (Some(FollowSpec::Regex(re)), vec![]),
                Err(e) => (
                    None,
                    vec![CheckWarning::InvalidFollowDirective {
                        directive: s.to_string(),
                        source: anyhow::anyhow!("invalid regex '{}': {:#}", rest, e),
                    }],
                ),
            }
        }
        "semver" => {
            if rest.is_empty() {
                return (None, vec![]);
            }
            match semver::VersionReq::parse(rest) {
                Ok(req) => (Some(FollowSpec::Semver(req)), vec![]),
                Err(e) => (
                    None,
                    vec![CheckWarning::InvalidFollowDirective {
                        directive: s.to_string(),
                        source: anyhow::anyhow!("invalid semver requirement '{}': {:#}", rest, e),
                    }],
                ),
            }
        }
        _ => (None, vec![]),
    }
}

pub(super) struct FollowResult {
    pub(super) sha: String,
}

pub(super) fn resolve_follow(
    spec: &FollowSpec,
    git_url: &str,
) -> (Option<FollowResult>, Vec<CheckWarning>) {
    match spec {
        FollowSpec::Branch(branch) => {
            let sha = match GitFetcher::get_latest_commit(git_url, branch) {
                Ok(Some(sha)) => sha,
                Ok(None) => {
                    return (
                        None,
                        vec![CheckWarning::FollowBranchNotFound {
                            git_url: git_url.to_string(),
                            branch: branch.clone(),
                        }],
                    );
                }
                Err(e) => {
                    return (
                        None,
                        vec![CheckWarning::FollowResolutionFailed {
                            git_url: git_url.to_string(),
                            source: e,
                        }],
                    );
                }
            };
            (Some(FollowResult { sha }), vec![])
        }
        FollowSpec::Regex(pattern) => {
            let tags = match GitFetcher::list_tags(git_url) {
                Ok(tags) => tags,
                Err(e) => {
                    return (
                        None,
                        vec![CheckWarning::FollowResolutionFailed {
                            git_url: git_url.to_string(),
                            source: e,
                        }],
                    );
                }
            };
            let matched: Vec<_> = tags
                .iter()
                .filter(|(name, _)| pattern.is_match(name))
                .collect();
            if matched.is_empty() {
                return (
                    None,
                    vec![CheckWarning::FollowRegexNoMatch {
                        git_url: git_url.to_string(),
                        pattern: pattern.to_string(),
                    }],
                );
            }
            let best = matched
                .into_iter()
                .max_by(|(a, _), (b, _)| VersionDetector::compare(a, b));
            match best {
                Some((_, sha)) => (Some(FollowResult { sha: sha.clone() }), vec![]),
                None => (None, vec![]),
            }
        }
        FollowSpec::Semver(requirement) => {
            let tags = match GitFetcher::list_tags(git_url) {
                Ok(tags) => tags,
                Err(e) => {
                    return (
                        None,
                        vec![CheckWarning::FollowResolutionFailed {
                            git_url: git_url.to_string(),
                            source: e,
                        }],
                    );
                }
            };
            let matched: Vec<_> = tags
                .iter()
                .filter(|(name, _)| {
                    let version_part = VersionDetector::prefix(name);
                    let stripped = name.strip_prefix(version_part).unwrap_or(name);
                    semver::Version::parse(stripped).is_ok_and(|v| requirement.matches(&v))
                })
                .collect();
            if matched.is_empty() {
                return (
                    None,
                    vec![CheckWarning::FollowSemverNoMatch {
                        git_url: git_url.to_string(),
                        requirement: requirement.to_string(),
                    }],
                );
            }
            let best = matched.into_iter().max_by(|(a, _), (b, _)| {
                let va =
                    semver::Version::parse(a.strip_prefix(VersionDetector::prefix(a)).unwrap_or(a));
                let vb =
                    semver::Version::parse(b.strip_prefix(VersionDetector::prefix(b)).unwrap_or(b));
                match (va, vb) {
                    (Ok(va), Ok(vb)) => va.cmp(&vb),
                    (Ok(_), Err(_)) => std::cmp::Ordering::Greater,
                    (Err(_), Ok(_)) => std::cmp::Ordering::Less,
                    (Err(_), Err(_)) => std::cmp::Ordering::Equal,
                }
            });
            match best {
                Some((_, sha)) => (Some(FollowResult { sha: sha.clone() }), vec![]),
                None => (None, vec![]),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_follow_spec_branch() {
        let (spec, warnings) = super::parse_follow_spec("branch main");
        assert!(matches!(spec, Some(super::FollowSpec::Branch(_))));
        assert!(warnings.is_empty());
        if let Some(super::FollowSpec::Branch(name)) = spec {
            assert_eq!(name, "main");
        }
    }

    #[test]
    fn test_parse_follow_spec_regex() {
        let (spec, warnings) = super::parse_follow_spec("regex v[0-9]+\\.[0-9]+");
        assert!(matches!(spec, Some(super::FollowSpec::Regex(_))));
        assert!(warnings.is_empty());
        if let Some(super::FollowSpec::Regex(re)) = spec {
            assert!(re.is_match("v1.0"));
            assert!(re.is_match("v2.41"));
            assert!(!re.is_match("2.41"));
        }
    }

    #[test]
    fn test_parse_follow_spec_regex_full_match() {
        let (spec, _) = super::parse_follow_spec("regex v[0-9]+\\.[0-9]+");
        if let Some(super::FollowSpec::Regex(re)) = spec {
            assert!(re.is_match("v1.0"));
            assert!(!re.is_match("v1.0.0"));
            assert!(!re.is_match("2.41"));
        }
    }

    #[test]
    fn test_parse_follow_spec_semver() {
        let (spec, _) = super::parse_follow_spec("semver 0.1");
        if let Some(super::FollowSpec::Semver(req)) = spec {
            assert!(req.matches(&semver::Version::parse("0.1.5").unwrap()));
            assert!(!req.matches(&semver::Version::parse("1.0.0").unwrap()));
        } else {
            panic!("expected Semver variant");
        }
    }

    #[test]
    fn test_parse_follow_spec_semver_gt() {
        let (spec, _) = super::parse_follow_spec("semver >0.1.0");
        if let Some(super::FollowSpec::Semver(req)) = spec {
            assert!(req.matches(&semver::Version::parse("0.2.0").unwrap()));
            assert!(!req.matches(&semver::Version::parse("0.1.0").unwrap()));
        } else {
            panic!("expected Semver variant");
        }
    }

    #[test]
    fn test_parse_follow_spec_semver_caret() {
        let (spec, _) = super::parse_follow_spec("semver ^0.1");
        if let Some(super::FollowSpec::Semver(req)) = spec {
            assert!(req.matches(&semver::Version::parse("0.1.5").unwrap()));
            assert!(!req.matches(&semver::Version::parse("1.0.0").unwrap()));
        } else {
            panic!("expected Semver variant");
        }
    }

    #[test]
    fn test_parse_follow_spec_invalid() {
        let (spec, warnings) = super::parse_follow_spec("unknown foo");
        assert!(spec.is_none());
        assert!(warnings.is_empty());
        assert!(super::parse_follow_spec("branch").0.is_none());
        assert!(super::parse_follow_spec("regex").0.is_none());
        assert!(super::parse_follow_spec("semver").0.is_none());
        assert!(super::parse_follow_spec("nonsense").0.is_none());
    }

    #[test]
    fn test_parse_follow_spec_invalid_regex() {
        let (result, warnings) = super::parse_follow_spec("regex [invalid");
        assert!(result.is_none());
        assert_eq!(warnings.len(), 1);
        assert!(matches!(
            warnings[0],
            crate::rules::traits::CheckWarning::InvalidFollowDirective { .. }
        ));
    }
}
