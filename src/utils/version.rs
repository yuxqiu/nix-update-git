use std::cmp::Ordering;

use versions::Versioning;

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

    pub fn latest<'a>(versions: &'a [&str]) -> Option<&'a str> {
        versions
            .iter()
            .filter(|v| Self::is_version(v))
            .max_by(|a, b| Self::compare(a, b))
            .copied()
    }

    fn parse(s: &str) -> Option<Versioning> {
        if !s.contains(|c: char| c.is_ascii_digit()) {
            return None;
        }
        Versioning::new(s)
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
        assert!(VersionDetector::is_version("1.2.3.4"));
        assert!(VersionDetector::is_version("25.01"));
        assert!(VersionDetector::is_version("2025.01"));
        assert!(VersionDetector::is_version("2025.01"));
        assert!(VersionDetector::is_version("2025.01.01"));
        assert!(VersionDetector::is_version("2025.01.01.123456"));
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
        assert_eq!(VersionDetector::compare("140.0", "141.0"), Ordering::Less);
        assert_eq!(VersionDetector::compare("100", "200"), Ordering::Less);
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
