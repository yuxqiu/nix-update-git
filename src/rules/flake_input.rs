use crate::parser::{AttrSpec, AttrType, NixNode, TextRange};
use crate::rules::traits::{Update, UpdateRule};
use crate::utils::{GitFetcher, VersionDetector};
use anyhow::Result;
use std::collections::HashMap;

const FLAKE_INPUT_ATTR_SPEC: &[AttrSpec] = &[
    AttrSpec {
        key: "url",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "ref",
        attr_type: AttrType::String,
    },
];

#[derive(Debug, Clone)]
enum FlakeUrl {
    GitHub { owner: String, repo: String },
    GitLab { owner: String, repo: String },
    SourceHut { owner: String, repo: String },
    GitRemote { url: String },
    GitLocal { path: String },
}

impl FlakeUrl {
    fn to_remote_url(&self) -> Option<String> {
        match self {
            FlakeUrl::GitHub { owner, repo } => {
                Some(format!("https://github.com/{}/{}", owner, repo))
            }
            FlakeUrl::GitLab { owner, repo } => {
                Some(format!("https://gitlab.com/{}/{}", owner, repo))
            }
            FlakeUrl::SourceHut { owner, repo } => {
                Some(format!("https://sr.ht/~{}/{}", owner, repo))
            }
            FlakeUrl::GitRemote { url } => Some(url.clone()),
            FlakeUrl::GitLocal { path } => Some(path.clone()),
        }
    }

    fn display_short(&self) -> String {
        match self {
            FlakeUrl::GitHub { owner, repo } => format!("{}/{}", owner, repo),
            FlakeUrl::GitLab { owner, repo } => format!("{}/{}", owner, repo),
            FlakeUrl::SourceHut { owner, repo } => format!("{}/{}", owner, repo),
            FlakeUrl::GitRemote { url } => url.clone(),
            FlakeUrl::GitLocal { path } => path.clone(),
        }
    }
}

struct ParsedFlakeUrl {
    flake_url: FlakeUrl,
    inline_ref: Option<String>,
}

#[derive(Debug)]
struct SourceValue {
    value: String,
    range: TextRange,
}

#[derive(Debug)]
struct InputDef {
    name: String,
    url: Option<SourceValue>,
    ref_value: Option<SourceValue>,
    inline_ref: bool,
    pinned: bool,
}

#[derive(Default)]
pub struct FlakeInputRule;

impl FlakeInputRule {
    fn parse_flake_url(url: &str) -> Option<ParsedFlakeUrl> {
        let url = url.trim();

        if let Some(rest) = url.strip_prefix("github:") {
            let (rest_without_ref, inline_ref) = Self::extract_ref_from_url(rest, true);
            let (owner, repo) = rest_without_ref.split_once('/')?;
            Some(ParsedFlakeUrl {
                flake_url: FlakeUrl::GitHub {
                    owner: owner.to_string(),
                    repo: repo.to_string(),
                },
                inline_ref,
            })
        } else if let Some(rest) = url.strip_prefix("gitlab:") {
            let (rest_without_ref, inline_ref) = Self::extract_ref_from_url(rest, true);
            let (owner, repo) = rest_without_ref.split_once('/')?;
            Some(ParsedFlakeUrl {
                flake_url: FlakeUrl::GitLab {
                    owner: owner.to_string(),
                    repo: repo.to_string(),
                },
                inline_ref,
            })
        } else if let Some(rest) = url.strip_prefix("sourcehut:") {
            let (rest_without_ref, inline_ref) = Self::extract_ref_from_url(rest, true);
            let (owner, repo) = rest_without_ref.split_once('/')?;
            Some(ParsedFlakeUrl {
                flake_url: FlakeUrl::SourceHut {
                    owner: owner.to_string(),
                    repo: repo.to_string(),
                },
                inline_ref,
            })
        } else if let Some(rest) = url.strip_prefix("git+https://") {
            let (clean_url, inline_ref) = Self::extract_ref_from_url(rest, false);
            Some(ParsedFlakeUrl {
                flake_url: FlakeUrl::GitRemote {
                    url: format!("https://{}", clean_url),
                },
                inline_ref,
            })
        } else if let Some(rest) = url.strip_prefix("git+ssh://") {
            let (clean_url, inline_ref) = Self::extract_ref_from_url(rest, false);
            Some(ParsedFlakeUrl {
                flake_url: FlakeUrl::GitRemote {
                    url: format!("ssh://{}", clean_url),
                },
                inline_ref,
            })
        } else if let Some(rest) = url.strip_prefix("git+file://") {
            let (clean_path, inline_ref) = Self::extract_ref_from_url(rest, false);
            Some(ParsedFlakeUrl {
                flake_url: FlakeUrl::GitLocal { path: clean_path },
                inline_ref,
            })
        } else if url.starts_with("git+http://") {
            let rest = url.strip_prefix("git+http://")?;
            let (clean_url, inline_ref) = Self::extract_ref_from_url(rest, false);
            Some(ParsedFlakeUrl {
                flake_url: FlakeUrl::GitRemote {
                    url: format!("http://{}", clean_url),
                },
                inline_ref,
            })
        } else {
            None
        }
    }

    fn extract_ref_from_url(rest: &str, trim_trailing_slash: bool) -> (String, Option<String>) {
        if let Some(qpos) = rest.find('?') {
            let mut base = &rest[..qpos];
            if trim_trailing_slash {
                base = base.trim_end_matches('/');
            }
            let query = &rest[qpos + 1..];
            let inline_ref = query
                .split('&')
                .find_map(|param| param.strip_prefix("ref="));
            (base.to_string(), inline_ref.map(String::from))
        } else {
            (rest.to_string(), None)
        }
    }

    fn reconstruct_url(original_url: &str, new_ref: &str) -> Option<String> {
        let url = original_url.trim();
        let parsed = Self::parse_flake_url(url)?;

        match &parsed.flake_url {
            FlakeUrl::GitHub { owner, repo } => {
                Some(format!("github:{}/{}?ref={}", owner, repo, new_ref))
            }
            FlakeUrl::GitLab { owner, repo } => {
                Some(format!("gitlab:{}/{}?ref={}", owner, repo, new_ref))
            }
            FlakeUrl::SourceHut { owner, repo } => {
                Some(format!("sourcehut:{}/{}?ref={}", owner, repo, new_ref))
            }
            FlakeUrl::GitRemote { url: remote_url } => {
                if url.starts_with("git+https://") {
                    Some(format!(
                        "git+https://{}?ref={}",
                        remote_url.trim_start_matches("https://"),
                        new_ref
                    ))
                } else if url.starts_with("git+ssh://") {
                    Some(format!(
                        "git+ssh://{}?ref={}",
                        remote_url.trim_start_matches("ssh://"),
                        new_ref
                    ))
                } else if url.starts_with("git+http://") {
                    Some(format!(
                        "git+http://{}?ref={}",
                        remote_url.trim_start_matches("http://"),
                        new_ref
                    ))
                } else {
                    None
                }
            }
            FlakeUrl::GitLocal { path } => Some(format!("git+file://{}?ref={}", path, new_ref)),
        }
    }

    fn collect_inputs_from_root(root: &NixNode) -> Vec<InputDef> {
        let mut inputs: HashMap<String, InputDef> = HashMap::new();

        for node in root.traverse() {
            if node.kind() != rnix::SyntaxKind::NODE_ATTRPATH_VALUE {
                continue;
            }

            let segments = node.attrpath_segments();
            if segments.is_empty() || segments[0] != "inputs" {
                continue;
            }

            match segments.len() {
                1 => {
                    if let Some(value) = node.attr_value()
                        && value.kind() == rnix::SyntaxKind::NODE_ATTR_SET
                    {
                        Self::collect_from_attr_set(&value, &mut inputs);
                    }
                }
                2 => {
                    let input_name = segments[1].clone();
                    if let Some(value) = node.attr_value() {
                        if value.kind() == rnix::SyntaxKind::NODE_ATTR_SET {
                            let parsed = match value.parse_attrs(FLAKE_INPUT_ATTR_SPEC, None) {
                                Ok(p) => p,
                                Err(_) => continue,
                            };
                            let pinned = value.has_pin_comment() || node.has_pin_comment();
                            let url = parsed.string_nodes.get("url").and_then(|n| {
                                n.pure_string_content().map(|v| SourceValue {
                                    value: v,
                                    range: n.text_range(),
                                })
                            });
                            let ref_value = parsed.string_nodes.get("ref").and_then(|n| {
                                n.pure_string_content().map(|v| SourceValue {
                                    value: v,
                                    range: n.text_range(),
                                })
                            });
                            inputs
                                .entry(input_name.clone())
                                .or_insert_with(|| InputDef {
                                    name: input_name.clone(),
                                    url,
                                    ref_value,
                                    inline_ref: false,
                                    pinned,
                                });
                        } else if value.kind() == rnix::SyntaxKind::NODE_STRING {
                            let range = value.text_range();
                            let url_content = value.pure_string_content();
                            let pinned = node.has_pin_comment();
                            if let Some(content) = url_content {
                                inputs
                                    .entry(input_name.clone())
                                    .or_insert_with(|| InputDef {
                                        name: input_name.clone(),
                                        url: Some(SourceValue {
                                            value: content,
                                            range,
                                        }),
                                        ref_value: None,
                                        inline_ref: false,
                                        pinned,
                                    });
                            }
                        }
                    }
                }
                3 => {
                    let input_name = segments[1].clone();
                    let attr = segments[2].clone();
                    let pinned = node.has_pin_comment();

                    if let Some(value_node) = node.attr_value()
                        && value_node.kind() == rnix::SyntaxKind::NODE_STRING
                    {
                        let range = value_node.text_range();
                        if let Some(content) = value_node.pure_string_content() {
                            let sv = SourceValue {
                                value: content,
                                range,
                            };
                            let def =
                                inputs
                                    .entry(input_name.clone())
                                    .or_insert_with(|| InputDef {
                                        name: input_name.clone(),
                                        url: None,
                                        ref_value: None,
                                        inline_ref: false,
                                        pinned,
                                    });

                            if attr == "url" {
                                def.url = Some(sv);
                            } else if attr == "ref" {
                                def.ref_value = Some(sv);
                            }
                        }
                    }
                    if pinned && let Some(def) = inputs.get_mut(&input_name) {
                        def.pinned = true;
                    }
                }
                _ => {}
            }
        }

        let mut result = inputs.into_values().collect::<Vec<_>>();

        for def in &mut result {
            if def.ref_value.is_none()
                && let Some(url) = &def.url
                && let Some(parsed) = Self::parse_flake_url(&url.value)
                && let Some(inline_ref) = parsed.inline_ref
            {
                def.inline_ref = true;
                def.ref_value = Some(SourceValue {
                    value: inline_ref,
                    range: url.range,
                });
            }
        }

        result
    }

    fn collect_from_attr_set(attr_set: &NixNode, inputs: &mut HashMap<String, InputDef>) {
        for entry in attr_set.children() {
            if entry.kind() != rnix::SyntaxKind::NODE_ATTRPATH_VALUE {
                continue;
            }

            let segments = entry.attrpath_segments();
            if segments.is_empty() {
                continue;
            }

            let input_name = segments[0].clone();

            if segments.len() == 1 {
                if let Some(value) = entry.attr_value() {
                    if value.kind() == rnix::SyntaxKind::NODE_ATTR_SET {
                        let parsed = match value.parse_attrs(FLAKE_INPUT_ATTR_SPEC, None) {
                            Ok(p) => p,
                            Err(_) => continue,
                        };
                        let pinned = value.has_pin_comment() || entry.has_pin_comment();
                        let url = parsed.string_nodes.get("url").and_then(|n| {
                            n.pure_string_content().map(|v| SourceValue {
                                value: v,
                                range: n.text_range(),
                            })
                        });
                        let ref_value = parsed.string_nodes.get("ref").and_then(|n| {
                            n.pure_string_content().map(|v| SourceValue {
                                value: v,
                                range: n.text_range(),
                            })
                        });
                        inputs
                            .entry(input_name.clone())
                            .or_insert_with(|| InputDef {
                                name: input_name.clone(),
                                url,
                                ref_value,
                                inline_ref: false,
                                pinned,
                            });
                    } else if value.kind() == rnix::SyntaxKind::NODE_STRING {
                        let range = value.text_range();
                        let url_content = value.pure_string_content();
                        let pinned = entry.has_pin_comment();
                        if let Some(content) = url_content {
                            inputs
                                .entry(input_name.clone())
                                .or_insert_with(|| InputDef {
                                    name: input_name.clone(),
                                    url: Some(SourceValue {
                                        value: content,
                                        range,
                                    }),
                                    ref_value: None,
                                    inline_ref: false,
                                    pinned,
                                });
                        }
                    }
                }
            } else if segments.len() == 2 {
                let attr = segments[1].clone();
                let pinned = entry.has_pin_comment();

                if let Some(value_node) = entry.attr_value()
                    && value_node.kind() == rnix::SyntaxKind::NODE_STRING
                {
                    let range = value_node.text_range();
                    if let Some(content) = value_node.pure_string_content() {
                        let sv = SourceValue {
                            value: content,
                            range,
                        };
                        let def = inputs
                            .entry(input_name.clone())
                            .or_insert_with(|| InputDef {
                                name: input_name.clone(),
                                url: None,
                                ref_value: None,
                                inline_ref: false,
                                pinned,
                            });

                        if attr == "url" {
                            def.url = Some(sv);
                        } else if attr == "ref" {
                            def.ref_value = Some(sv);
                        }
                    }
                }
                if pinned && let Some(def) = inputs.get_mut(&input_name) {
                    def.pinned = true;
                }
            }
        }
    }
}

impl UpdateRule for FlakeInputRule {
    fn name(&self) -> &str {
        "flake-input"
    }

    fn matches(&self, node: &NixNode) -> bool {
        node.kind() == rnix::SyntaxKind::NODE_ROOT
    }

    fn check(&self, node: &NixNode) -> Result<Option<Vec<Update>>> {
        let mut updates = Vec::new();

        let root_attrs = match Self::find_root_attr_set(node) {
            Some(attrs) => attrs,
            None => return Ok(None),
        };

        let input_defs = Self::collect_inputs_from_root(&root_attrs);

        for input_def in input_defs {
            if input_def.pinned {
                continue;
            }

            let ref_sv = match &input_def.ref_value {
                Some(sv) => sv,
                None => continue,
            };

            if !VersionDetector::is_version(&ref_sv.value) {
                continue;
            }

            let url_sv = match &input_def.url {
                Some(sv) => sv,
                None => continue,
            };

            let parsed = match Self::parse_flake_url(&url_sv.value) {
                Some(p) => p,
                None => continue,
            };

            let remote_url = match parsed.flake_url.to_remote_url() {
                Some(u) => u,
                None => continue,
            };

            let detail = parsed.flake_url.display_short();

            if let Ok(Some(latest_tag)) =
                GitFetcher::get_latest_tag_matching(&remote_url, Some(&ref_sv.value))
                && VersionDetector::compare(&ref_sv.value, &latest_tag) == std::cmp::Ordering::Less
            {
                if input_def.inline_ref {
                    if let Some(new_url) = Self::reconstruct_url(&url_sv.value, &latest_tag) {
                        updates.push(
                            Update::new(
                                format!("inputs.{}.url", input_def.name),
                                format!("\"{}\"", new_url),
                                url_sv.range,
                            )
                            .with_detail(detail),
                        );
                    }
                } else {
                    updates.push(
                        Update::new(
                            format!("inputs.{}.ref", input_def.name),
                            format!("\"{}\"", latest_tag),
                            ref_sv.range,
                        )
                        .with_detail(detail),
                    );
                }
            }
        }

        if updates.is_empty() {
            Ok(None)
        } else {
            Ok(Some(updates))
        }
    }
}

impl FlakeInputRule {
    fn find_root_attr_set(node: &NixNode) -> Option<NixNode> {
        if node.kind() == rnix::SyntaxKind::NODE_ATTR_SET {
            return Some(node.clone());
        }
        for child in node.children() {
            if child.kind() == rnix::SyntaxKind::NODE_ATTR_SET {
                return Some(child);
            }
            if let Some(found) = Self::find_root_attr_set(&child) {
                return Some(found);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_github_url() {
        let result = FlakeInputRule::parse_flake_url("github:NixOS/nixpkgs").unwrap();
        assert!(matches!(result.flake_url, FlakeUrl::GitHub { .. }));
        if let FlakeUrl::GitHub { owner, repo } = result.flake_url {
            assert_eq!(owner, "NixOS");
            assert_eq!(repo, "nixpkgs");
        }
        assert!(result.inline_ref.is_none());
    }

    #[test]
    fn test_parse_gitlab_url() {
        let result = FlakeInputRule::parse_flake_url("gitlab:foo/bar").unwrap();
        assert!(matches!(result.flake_url, FlakeUrl::GitLab { .. }));
        assert!(result.inline_ref.is_none());
    }

    #[test]
    fn test_parse_sourcehut_url() {
        let result = FlakeInputRule::parse_flake_url("sourcehut:~user/repo").unwrap();
        assert!(matches!(result.flake_url, FlakeUrl::SourceHut { .. }));
        assert!(result.inline_ref.is_none());
    }

    #[test]
    fn test_parse_git_https_url() {
        let result = FlakeInputRule::parse_flake_url("git+https://example.com/repo.git").unwrap();
        assert!(matches!(result.flake_url, FlakeUrl::GitRemote { .. }));
        assert!(result.inline_ref.is_none());
    }

    #[test]
    fn test_parse_git_ssh_url() {
        let result = FlakeInputRule::parse_flake_url("git+ssh://git@example.com/repo.git").unwrap();
        assert!(matches!(result.flake_url, FlakeUrl::GitRemote { .. }));
    }

    #[test]
    fn test_parse_git_file_url() {
        let result = FlakeInputRule::parse_flake_url("git+file:///path/to/repo").unwrap();
        assert!(matches!(result.flake_url, FlakeUrl::GitLocal { .. }));
    }

    #[test]
    fn test_parse_git_url_strips_query_params() {
        let result =
            FlakeInputRule::parse_flake_url("git+https://example.com/repo.git?ref=v1.0").unwrap();
        if let FlakeUrl::GitRemote { url } = result.flake_url {
            assert_eq!(url, "https://example.com/repo.git");
        } else {
            panic!("Expected GitRemote");
        }
        assert_eq!(result.inline_ref.as_deref(), Some("v1.0"));
    }

    #[test]
    fn test_parse_github_url_with_ref() {
        let result =
            FlakeInputRule::parse_flake_url("github:gmodena/nix-flatpak/?ref=v0.6.0").unwrap();
        if let FlakeUrl::GitHub { owner, repo } = result.flake_url {
            assert_eq!(owner, "gmodena");
            assert_eq!(repo, "nix-flatpak");
        } else {
            panic!("Expected GitHub");
        }
        assert_eq!(result.inline_ref.as_deref(), Some("v0.6.0"));
    }

    #[test]
    fn test_parse_gitlab_url_with_ref() {
        let result = FlakeInputRule::parse_flake_url("gitlab:foo/bar?ref=v1.0.0").unwrap();
        if let FlakeUrl::GitLab { owner, repo } = result.flake_url {
            assert_eq!(owner, "foo");
            assert_eq!(repo, "bar");
        } else {
            panic!("Expected GitLab");
        }
        assert_eq!(result.inline_ref.as_deref(), Some("v1.0.0"));
    }

    #[test]
    fn test_parse_sourcehut_url_with_ref() {
        let result = FlakeInputRule::parse_flake_url("sourcehut:~user/repo?ref=v2.0").unwrap();
        if let FlakeUrl::SourceHut { owner, repo } = result.flake_url {
            assert_eq!(owner, "~user");
            assert_eq!(repo, "repo");
        } else {
            panic!("Expected SourceHut");
        }
        assert_eq!(result.inline_ref.as_deref(), Some("v2.0"));
    }

    #[test]
    fn test_gitlocal_to_remote_url() {
        let result = FlakeInputRule::parse_flake_url("git+file:///tmp/repo");
        assert!(result.is_some());
        if let Some(parsed) = result {
            assert!(parsed.flake_url.to_remote_url().is_some());
        }
    }

    #[test]
    fn test_reconstruct_github_url() {
        let new_url =
            FlakeInputRule::reconstruct_url("github:gmodena/nix-flatpak/?ref=v0.6.0", "v0.7.0");
        assert_eq!(
            new_url,
            Some("github:gmodena/nix-flatpak?ref=v0.7.0".to_string())
        );
    }

    #[test]
    fn test_reconstruct_github_url_without_ref() {
        let new_url = FlakeInputRule::reconstruct_url("github:NixOS/nixpkgs", "v2.0.0");
        assert_eq!(new_url, Some("github:NixOS/nixpkgs?ref=v2.0.0".to_string()));
    }

    #[test]
    fn test_reconstruct_git_https_url() {
        let new_url =
            FlakeInputRule::reconstruct_url("git+https://example.com/repo.git?ref=v1.0", "v2.0");
        assert_eq!(
            new_url,
            Some("git+https://example.com/repo.git?ref=v2.0".to_string())
        );
    }

    #[test]
    fn test_parse_plain_url_returns_none() {
        assert!(FlakeInputRule::parse_flake_url("https://example.com/repo").is_none());
    }

    #[test]
    fn test_parse_github_url_trailing_slash() {
        let result = FlakeInputRule::parse_flake_url("github:owner/repo/").unwrap();
        if let FlakeUrl::GitHub { owner, repo } = result.flake_url {
            assert_eq!(owner, "owner");
            assert_eq!(repo, "repo/");
        } else {
            panic!("Expected GitHub");
        }
        assert_eq!(result.inline_ref, None);
    }

    #[test]
    fn test_parse_git_https_without_ref() {
        let result = FlakeInputRule::parse_flake_url("git+https://example.com/repo.git").unwrap();
        if let FlakeUrl::GitRemote { url } = result.flake_url {
            assert_eq!(url, "https://example.com/repo.git");
        } else {
            panic!("Expected GitRemote");
        }
        assert_eq!(result.inline_ref, None);
    }

    #[test]
    fn test_parse_url_with_multiple_query_params() {
        let result =
            FlakeInputRule::parse_flake_url("github:owner/repo?ref=v1&rev=abc123&submodules=true")
                .unwrap();
        if let FlakeUrl::GitHub { owner, repo } = result.flake_url {
            assert_eq!(owner, "owner");
            assert_eq!(repo, "repo");
        } else {
            panic!("Expected GitHub");
        }
        assert_eq!(result.inline_ref.as_deref(), Some("v1"));
    }

    #[test]
    fn test_reconstruct_url_round_trip() {
        let test_cases = vec![
            "github:owner/repo?ref=v1.0",
            "gitlab:foo/bar?ref=main",
            "sourcehut:~user/repo?ref=dev",
            "git+https://example.com/repo.git?ref=master",
            "git+ssh://git@example.com/repo.git?ref=main",
        ];

        for url in test_cases {
            let parsed = FlakeInputRule::parse_flake_url(url).unwrap();
            let ref_value = parsed.inline_ref.as_deref().unwrap_or("new-ref");
            let reconstructed = FlakeInputRule::reconstruct_url(url, ref_value).unwrap();
            let _reparsed = FlakeInputRule::parse_flake_url(&reconstructed).unwrap();
        }
    }
}
