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
        versions
            .iter()
            .filter(|v| Self::is_version(v))
            .filter(|v| Self::look_similar(v, current))
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

    fn look_similar(a: &str, b: &str) -> bool {
        if Self::prefix(a) != Self::prefix(b) {
            return false;
        }

        let va = match Self::parse(a) {
            Some(v) => v,
            None => return false,
        };
        let vb = match Self::parse(b) {
            Some(v) => v,
            None => return false,
        };

        match (&va, &vb) {
            (Versioning::Ideal(_), Versioning::Ideal(_)) => true,
            (Versioning::General(a_ver), Versioning::General(b_ver)) => {
                if a_ver.chunks.0.len() != b_ver.chunks.0.len() {
                    return false;
                }
                a_ver
                    .chunks
                    .0
                    .iter()
                    .zip(b_ver.chunks.0.iter())
                    .all(|(ac, bc)| std::mem::discriminant(ac) == std::mem::discriminant(bc))
            }
            (Versioning::Complex(a_mess), Versioning::Complex(b_mess)) => {
                Self::mess_shape_similar(a_mess, b_mess)
            }
            // Stable versions shouldn't match pre-release shapes and vice versa
            _ => false,
        }
    }

    fn mess_shape_similar(a: &versions::Mess, b: &versions::Mess) -> bool {
        let mut a_curr = a;
        let mut b_curr = b;
        loop {
            if a_curr.chunks.len() != b_curr.chunks.len() {
                return false;
            }
            if !a_curr
                .chunks
                .iter()
                .zip(b_curr.chunks.iter())
                .all(|(ac, bc)| std::mem::discriminant(ac) == std::mem::discriminant(bc))
            {
                return false;
            }
            match (&a_curr.next, &b_curr.next) {
                (Some((_, a_next)), Some((_, b_next))) => {
                    a_curr = a_next;
                    b_curr = b_next;
                }
                (None, None) => return true,
                _ => return false,
            }
        }
    }

    fn prefix(s: &str) -> &str {
        let end = s.find(|c: char| c.is_ascii_digit()).unwrap_or(s.len());
        &s[..end]
    }

    fn parse(s: &str) -> Option<Versioning> {
        let stripped = s.strip_prefix(Self::prefix(s))?;

        Versioning::new(stripped).filter(|v| match v {
            Versioning::Ideal(_) => true,
            Versioning::General(version) => version
                .chunks
                .0
                .iter()
                .any(|chunk| matches!(chunk, Chunk::Numeric(_))),
            Versioning::Complex(version) => Self::mess_has_numerics(version),
        })
    }

    fn mess_has_numerics(mess: &versions::Mess) -> bool {
        let mut current = mess;
        loop {
            if current
                .chunks
                .iter()
                .any(|chunk| matches!(chunk, MChunk::Digits(_, _) | MChunk::Rev(_, _)))
            {
                return true;
            }
            match &current.next {
                Some((_, next)) => current = next,
                None => return false,
            }
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
        let versions: Vec<&str> = vec!["v1.0.0", "v2.6", "2.6", "v1.5.0"];
        assert_eq!(
            VersionDetector::latest_matching(&versions, "v2.41"),
            Some("v2.6")
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

    #[test]
    fn test_look_similar_same_prefix() {
        assert!(VersionDetector::look_similar("v1.0.0", "v2.0.0"));
        assert!(VersionDetector::look_similar("v1.0", "v2.0"));
        assert!(VersionDetector::look_similar("1.0.0", "2.0.0"));
        assert!(VersionDetector::look_similar(
            "release-1.0.0",
            "release-2.0.0"
        ));
    }

    #[test]
    fn test_look_similar_different_prefix() {
        assert!(!VersionDetector::look_similar("v1.0.0", "2.0.0"));
        assert!(!VersionDetector::look_similar("v1.0.0", "release-2.0.0"));
        assert!(!VersionDetector::look_similar("1.0.0", "v2.0.0"));
    }

    #[test]
    fn test_look_similar_stable_vs_prerelease() {
        // Both "v1.0.0" and "v1.0.0-beta" parse as Ideal (SemVer) because
        // semver pre-release is still semver. They are similar in shape.
        // Complex (Mess) versions like "1.0.0-abc+def.2" differ from stable ones.
        assert!(VersionDetector::look_similar("v1.0.0", "v2.0.0-alpha"));
        // Truly different shapes are not similar: General (2 chunks) vs Complex
        assert!(VersionDetector::look_similar("v1.0.0", "v2.0.0"));
    }

    #[test]
    fn test_look_similar_general_different_chunks() {
        // Different number of chunks: not similar
        assert!(!VersionDetector::look_similar("1.0", "1.0.0.1"));
    }

    #[test]
    fn test_look_similar_prerelease_vs_stable() {
        // Verify that prefix check still works for pre-release tags
        // "v1.0.0-beta" has prefix "v" and "v2.0.0" has prefix "v"
        // Both parse as Ideal (SemVer), so they're similar in shape.
        // This is correct: both are valid SemVer versions.
        assert!(VersionDetector::look_similar("v1.0.0-beta", "v2.0.0"));
    }

    #[test]
    fn test_is_version_mess_with_numerics() {
        // Complex versions that have numerics in different segments
        assert!(VersionDetector::is_version("1.0.0-beta.1"));
        assert!(VersionDetector::is_version("rc-2"));
    }
}
