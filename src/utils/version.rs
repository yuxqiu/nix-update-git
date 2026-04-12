use semver::Version;
use std::cmp::Ordering;

pub struct VersionDetector;

impl VersionDetector {
    pub fn is_version(s: &str) -> bool {
        Self::parse_flexible(s).is_some()
    }

    pub fn compare(a: &str, b: &str) -> Ordering {
        let version_a = Self::parse_version(a);
        let version_b = Self::parse_version(b);
        version_a.cmp(&version_b)
    }

    pub fn latest<'a>(versions: &'a [&str]) -> Option<&'a str> {
        versions
            .iter()
            .filter(|v| Self::is_version(v))
            .max_by(|a, b| Self::compare(a, b))
            .copied()
    }

    fn parse_flexible(s: &str) -> Option<Version> {
        let stripped = s.strip_prefix('v').unwrap_or(s);

        if let Ok(v) = Version::parse(stripped) {
            return Some(v);
        }

        let parts: Vec<&str> = stripped.split('.').collect();
        if parts.is_empty() || parts.len() > 3 {
            return None;
        }

        if !parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())) {
            return None;
        }

        let padded = match parts.len() {
            1 => format!("{}.0.0", stripped),
            2 => format!("{}.0", stripped),
            _ => stripped.to_string(),
        };

        Version::parse(&padded).ok()
    }

    fn parse_version(s: &str) -> Version {
        if let Some(v) = Self::parse_flexible(s) {
            v
        } else {
            Version::new(0, 0, 0)
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
        assert!(VersionDetector::is_version("140.0"));
        assert!(VersionDetector::is_version("v140.0"));
        assert!(VersionDetector::is_version("100"));
        assert!(!VersionDetector::is_version("1.2.3.4"));
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
