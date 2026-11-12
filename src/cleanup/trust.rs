//! Fixed executable candidates for future trust enrollment.
//!
//! This is deliberately metadata only: it neither executes nor enrolls a file.
//! A candidate path is not evidence that an executable is trustworthy.

/// Candidate names are explicit; never search PATH or launch a candidate here.
pub fn candidates(provider: &str) -> Option<&'static [&'static str]> {
    match provider {
        "pip" => Some(&["/opt/homebrew/bin/pip3", "/usr/local/bin/pip3"]),
        "go" => Some(&["/opt/homebrew/bin/go", "/usr/local/bin/go"]),
        "homebrew" => Some(&["/opt/homebrew/bin/brew", "/usr/local/bin/brew"]),
        "npm" => Some(&["/opt/homebrew/bin/npm", "/usr/local/bin/npm"]),
        "yarn" => Some(&["/opt/homebrew/bin/yarn", "/usr/local/bin/yarn"]),
        "poetry" => Some(&["/opt/homebrew/bin/poetry", "/usr/local/bin/poetry"]),
        "composer" => Some(&["/opt/homebrew/bin/composer", "/usr/local/bin/composer"]),
        "cocoapods" => Some(&["/opt/homebrew/bin/pod", "/usr/local/bin/pod"]),
        "cargo" => Some(&["/opt/homebrew/bin/cargo", "/usr/local/bin/cargo"]),
        "gradle" => Some(&["/opt/homebrew/bin/gradle", "/usr/local/bin/gradle"]),
        _ => None,
    }
}

/// Persisted identity data; constructing a record does not authorize execution.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustRecord {
    pub provider: String,
    pub path: std::path::PathBuf,
    pub device: u64,
    pub inode: u64,
    pub size: u64,
    pub mtime_seconds: i64,
    pub mtime_nanoseconds: u32,
    pub sha256: String,
    pub signature_identity: Option<String>,
    pub version: String,
    pub enrolled_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustStore {
    pub schema_version: u32,
    pub records: Vec<TrustRecord>,
}

impl TrustStore {
    /// Reject malformed and unsupported data; never silently reset it.
    pub fn parse(bytes: &[u8]) -> std::io::Result<Self> {
        let invalid = |message: &str| std::io::Error::new(std::io::ErrorKind::InvalidData, message);
        if bytes.len() > 1_048_576 {
            return Err(invalid("trust store exceeds 1 MiB"));
        }
        let store: Self =
            serde_json::from_slice(bytes).map_err(|_| invalid("malformed trust store"))?;
        if store.schema_version != 1 {
            return Err(invalid("unsupported trust store schema"));
        }
        let mut providers = std::collections::HashSet::new();
        for r in &store.records {
            if candidates(&r.provider).is_none()
                || !providers.insert(&r.provider)
                || !r.path.is_absolute()
                || r.path.components().any(|c| {
                    !matches!(
                        c,
                        std::path::Component::RootDir | std::path::Component::Normal(_)
                    )
                })
                || r.mtime_nanoseconds >= 1_000_000_000
                || r.sha256.len() != 64
                || !r
                    .sha256
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                || r.version.trim().is_empty()
                || r.version.chars().any(char::is_control)
                || r.signature_identity
                    .as_ref()
                    .is_some_and(|s| s.is_empty() || s.chars().any(char::is_control))
            {
                return Err(invalid("invalid trust record"));
            }
        }
        Ok(store)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn store_parser_rejects_corruption_and_future_schema() {
        assert!(TrustStore::parse(br#"{invalid"#).is_err());
        assert!(TrustStore::parse(br#"{"schema_version":2,"records":[]}"#).is_err());
        assert!(TrustStore::parse(br#"{"schema_version":1,"records":[]}"#).is_ok());
    }
    #[test]
    fn schema_round_trip_and_malformed_records() {
        let record = TrustRecord {
            provider: "go".into(),
            path: "/Users/example/bin/go".into(),
            device: 1,
            inode: 2,
            size: 3,
            mtime_seconds: 4,
            mtime_nanoseconds: 5,
            sha256: "a".repeat(64),
            signature_identity: None,
            version: "go fixture".into(),
            enrolled_at: 6,
        };
        let store = TrustStore {
            schema_version: 1,
            records: vec![record],
        };
        let encoded = serde_json::to_vec(&store).unwrap();
        assert_eq!(TrustStore::parse(&encoded).unwrap(), store);
        let mut malformed = store.clone();
        malformed.records[0].sha256 = "invalid".into();
        assert!(TrustStore::parse(&serde_json::to_vec(&malformed).unwrap()).is_err());
        malformed = store.clone();
        malformed.records.push(store.records[0].clone());
        assert!(TrustStore::parse(&serde_json::to_vec(&malformed).unwrap()).is_err());
    }
    #[test]
    fn candidates_are_absolute_and_unknown_providers_are_rejected() {
        for provider in [
            "pip",
            "go",
            "homebrew",
            "npm",
            "yarn",
            "poetry",
            "composer",
            "cocoapods",
            "cargo",
            "gradle",
        ] {
            assert!(
                candidates(provider)
                    .unwrap()
                    .iter()
                    .all(|p| std::path::Path::new(p).is_absolute())
            );
        }
        assert!(candidates("../../bin/sh").is_none());
    }
}
