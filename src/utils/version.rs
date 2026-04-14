use std::cmp::Ordering;

use versions::{Chunk, MChunk, Versioning};

pub struct VersionDetector;

impl VersionDetector {
    pub fn is_version(s: &str) -> bool {
        Self::parse(s).is_some()
    }

    pub fn compare(a: &str, b: &str) -> Ordering {
        let version_a = Self::parse(a);
        let version_b = Self::parse(b);
        match (version_a, version_b) {
            (Some(a), Some(b)) => a.cmp(&b),
            (Some(_), None) => Ordering::Greater,
            (None, Some(_)) => Ordering::Less,
            (None, None) => Ordering::Equal,
        }
    }

    pub fn latest_matching<'a>(versions: &'a [&str], current: &str) -> Option<&'a str> {
        let prefix = Self::prefix(current);
        versions
            .iter()
            .filter(|v| Self::is_version(v))
            .filter(|v| Self::prefix(v) == prefix)
            .max_by(|a, b| Self::compare(a, b))
            .copied()
    }

    pub fn latest<'a>(versions: &'a [&str]) -> Option<&'a str> {
        versions
            .iter()
            .filter(|v| Self::is_version(v))
            .max_by(|a, b| Self::compare(a, b))
            .copied()
    }

    fn prefix(s: &str) -> &str {
        let end = s.find(|c: char| c.is_ascii_digit()).unwrap_or(s.len());
        &s[..end]
    }

    fn parse(s: &str) -> Option<Versioning> {
        let stripped = s.strip_prefix(Self::prefix(s)).unwrap();

        // accept if
        // - the version is semver
        // - the version has any numeric components
        Versioning::new(stripped).filter(|v| match v {
            Versioning::Ideal(_) => true,
            Versioning::General(version) => version
                .chunks
                .0
                .iter()
                .find(|chunk| matches!(chunk, Chunk::Numeric(_)))
                .is_some(),
            Versioning::Complex(version) => version
                .chunks
                .iter()
                .find(|chunk| {
                    matches!(chunk, MChunk::Digits(_, _)) || matches!(chunk, MChunk::Rev(_, _))
                })
                .is_some(),
        })
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
        assert!(VersionDetector::is_version("140.0"));
        assert!(VersionDetector::is_version("v140.0"));
        assert!(VersionDetector::is_version("100"));
        assert!(VersionDetector::is_version("100"));

        assert!(!VersionDetector::is_version("main"));
        assert!(!VersionDetector::is_version("master"));
        assert!(!VersionDetector::is_version(
            "0f14e030b3a9391e761c03ce3c260730a78a4db6"
        ));

        assert!(VersionDetector::is_version("25.01"));
        assert!(VersionDetector::is_version("2025.01"));
        assert!(VersionDetector::is_version("2025.01.01"));
        assert!(VersionDetector::is_version("2025.01.01.123456"));

        assert!(VersionDetector::is_version("1.2.3.4"));
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
        assert_eq!(VersionDetector::compare("140.0", "141.0"), Ordering::Less);
        assert_eq!(VersionDetector::compare("100", "200"), Ordering::Less);
        assert_eq!(VersionDetector::compare("2.41", "2.6"), Ordering::Greater);
    }

    #[test]
    fn test_prefix() {
        assert_eq!(VersionDetector::prefix("v1.0.0"), "v");
        assert_eq!(VersionDetector::prefix("V2.0"), "V");
        assert_eq!(VersionDetector::prefix("1.0.0"), "");
        assert_eq!(VersionDetector::prefix("140.0"), "");
        assert_eq!(VersionDetector::prefix("release-1.0.0"), "release-");
        assert_eq!(VersionDetector::prefix("rc-2.0-beta"), "rc-");
    }

    #[test]
    fn test_latest_matching() {
        let versions: Vec<&str> = vec!["v1.0.0", "v2.0.0", "2.6", "v1.5.0"];
        assert_eq!(
            VersionDetector::latest_matching(&versions, "v2.41"),
            Some("v2.0.0")
        );
        assert_eq!(
            VersionDetector::latest_matching(&versions, "2.41"),
            Some("2.6")
        );

        let versions: Vec<&str> = vec!["v1.0.0", "v2.0.0", "v1.5.0"];
        assert_eq!(
            VersionDetector::latest_matching(&versions, "v1.0.0"),
            Some("v2.0.0")
        );

        let versions: Vec<&str> = vec!["1.0.0", "2.0.0", "1.5.0"];
        assert_eq!(
            VersionDetector::latest_matching(&versions, "1.0.0"),
            Some("2.0.0")
        );

        let versions: Vec<&str> = vec!["release-1.0.0", "release-2.0.0", "v3.0.0"];
        assert_eq!(
            VersionDetector::latest_matching(&versions, "release-1.0.0"),
            Some("release-2.0.0")
        );
    }

    #[test]
    fn test_latest() {
        let versions: Vec<&str> = vec!["v1.0.0", "v2.0.0", "v1.5.0"];
        let result = VersionDetector::latest(&versions);
        assert_eq!(result, Some("v2.0.0"));

        let versions: Vec<&str> = vec!["100.0", "140.0", "141.0"];
        let result = VersionDetector::latest(&versions);
        assert_eq!(result, Some("141.0"));
    }
}
