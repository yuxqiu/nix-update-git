use semver::{Version, VersionReq};
use std::cmp::Ordering;

pub struct VersionDetector;

impl VersionDetector {
    pub fn is_version(s: &str) -> bool {
        if let Some(stripped) = s.strip_prefix('v') {
            Version::parse(stripped).is_ok()
        } else {
            Version::parse(s).is_ok()
        }
    }

    pub fn compare(a: &str, b: &str) -> Ordering {
        let version_a = Self::parse_version(a);
        let version_b = Self::parse_version(b);
        version_a.cmp(&version_b)
    }

    pub fn latest<'a>(versions: &'a [&str]) -> Option<&'a str> {
        versions.iter().max_by(|a, b| Self::compare(a, b)).copied()
    }

    fn parse_version(s: &str) -> Version {
        let stripped = s.strip_prefix('v').unwrap_or(s);
        Version::parse(stripped).unwrap_or_else(|_| Version::new(0, 0, 0))
    }

    pub fn satisfies(version: &str, requirement: &str) -> bool {
        let version = Self::parse_version(version);
        if let Ok(req) = VersionReq::parse(requirement) {
            req.matches(&version)
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn test_is_version() {
        assert!(VersionDetector::is_version("v1.0.0"));
        assert!(VersionDetector::is_version("1.0.0"));
        assert!(VersionDetector::is_version("0.1.0-beta"));
        assert!(!VersionDetector::is_version("main"));
        assert!(!VersionDetector::is_version("master"));
    }

    #[test]
    fn test_compare() {
        assert_eq!(VersionDetector::compare("v1.0.0", "v2.0.0"), Ordering::Less);
        assert_eq!(
            VersionDetector::compare("v2.0.0", "v1.0.0"),
            Ordering::Greater
        );
        assert_eq!(
            VersionDetector::compare("v1.0.0", "v1.0.0"),
            Ordering::Equal
        );
        assert_eq!(VersionDetector::compare("v1.0.0", "v1.0.1"), Ordering::Less);
    }

    #[test]
    fn test_latest() {
        let versions: Vec<&str> = vec!["v1.0.0", "v2.0.0", "v1.5.0"];
        let result = VersionDetector::latest(&versions);
        assert_eq!(result, Some("v2.0.0"));
    }
}
