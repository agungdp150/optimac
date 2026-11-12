//! Versioned plan-directory-unit recovery records (record schema 2).
//!
//! Version-1 records stay readable and are never rewritten. A directory unit is
//! moved only by the existing exclusive same-volume rename: there is no
//! copy/delete fallback, no automatic replay after interruption and no purge.
//! Internal symlinks travel with the directory; they are never read or followed.
use super::{Record, error};
use serde::{Deserialize, Serialize};
use std::{
    io,
    path::{Component, Path, PathBuf},
};

pub const UNIT_RECORD_VERSION: u32 = 2;
pub const UNIT_RECORD_KIND: &str = "plan_directory_unit";
const PAYLOAD: &str = "payload";
const MAX_RECORD_BYTES: usize = 65536;

/// A durable record for one top-level plan unit moved into recovery.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitRecord {
    /// Record schema version (2). Version-1 readers fail closed on this record.
    pub version: u32,
    pub kind: String,
    pub original_path: PathBuf,
    pub provider: String,
    pub plan_id: String,
    pub device: u64,
    pub inode: u64,
    pub owner: u32,
    pub file_type: String,
    pub allocated_bytes: u64,
    pub logical_bytes: u64,
    pub created_at_unix_seconds: u64,
    /// Payload location relative to the record directory.
    pub payload: String,
}

impl UnitRecord {
    /// `identity` is (device, inode, owner); `bytes` is (allocated, logical).
    pub fn directory(
        original_path: PathBuf,
        provider: &str,
        plan_id: &str,
        identity: (u64, u64, u32),
        bytes: (u64, u64),
        created_at_unix_seconds: u64,
    ) -> Self {
        Self {
            version: UNIT_RECORD_VERSION,
            kind: UNIT_RECORD_KIND.into(),
            original_path,
            provider: provider.into(),
            plan_id: plan_id.into(),
            device: identity.0,
            inode: identity.1,
            owner: identity.2,
            file_type: "directory".into(),
            allocated_bytes: bytes.0,
            logical_bytes: bytes.1,
            created_at_unix_seconds,
            payload: PAYLOAD.into(),
        }
    }

    /// Scope checks shared by move and restore. A record cannot nominate a
    /// destination outside home, inside recovery or under a protected name.
    pub fn validate(&self, home: &Path, recovery: &Path) -> io::Result<()> {
        let path = &self.original_path;
        if self.version != UNIT_RECORD_VERSION
            || self.kind != UNIT_RECORD_KIND
            || self.file_type != "directory"
            || self.payload != PAYLOAD
            || !crate::cleanup::plan_builder::ALLOWED.contains(&self.provider.as_str())
            || self.plan_id.len() != 64
            || !self
                .plan_id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(error("Malformed directory-unit record"));
        }
        if !path.is_absolute()
            || path
                .components()
                .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
            || !path.starts_with(home)
            || path == home
            || path.parent() == Some(home)
            || path.starts_with(recovery)
            || crate::cleanup::protected_path(path)
        {
            return Err(error("Directory-unit path outside the allowed scope"));
        }
        Ok(())
    }
}

/// Any record a recovery directory may hold.
#[derive(Debug)]
pub(super) enum StoredRecord {
    V1(Record),
    Unit(UnitRecord),
}

/// Dispatch on the record's version before decoding its body.
pub(super) fn parse_record(bytes: &[u8]) -> io::Result<StoredRecord> {
    #[derive(Deserialize)]
    struct Header {
        version: u32,
    }
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(error("Oversized recovery record"));
    }
    let header: Header = serde_json::from_slice(bytes)?;
    match header.version {
        1 => Ok(StoredRecord::V1(serde_json::from_slice(bytes)?)),
        UNIT_RECORD_VERSION => {
            let record: UnitRecord = serde_json::from_slice(bytes)?;
            if record.kind != UNIT_RECORD_KIND {
                return Err(error("Unknown recovery record kind"));
            }
            Ok(StoredRecord::Unit(record))
        }
        _ => Err(error("Unsupported recovery version")),
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use crate::{
        fixture_paths::FixtureRoot,
        recovery::{RecoveryContext, RecoveryPolicy},
    };
    use std::{fs, os::unix::fs::MetadataExt};

    const PLAN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn recovery_record_versions_load_v1_and_unit_records_fail_closed() {
        let (home, cap) = FixtureRoot::create_test_fixture().unwrap();
        let context =
            RecoveryContext::initialize(&home, RecoveryPolicy::Fixture(Some(&cap))).unwrap();
        let lock = context.lock().unwrap();
        let legacy = home.join(".cache/pip/legacy");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(&legacy, b"v1").unwrap();
        let m = fs::metadata(&legacy).unwrap();
        let candidate: crate::recovery::Candidate = serde_json::from_value(serde_json::json!({
            "path": legacy, "dev": m.dev(), "ino": m.ino(), "size": m.len(), "modified": m.mtime()
        }))
        .unwrap();
        context.create_journal("5001", &candidate).unwrap();
        drop(lock);
        let stored = fs::read(home.join(".optimac-rust-recovery/5001/record.json")).unwrap();
        let StoredRecord::V1(old) = parse_record(&stored).unwrap() else {
            panic!("v1 record must still load as v1");
        };
        assert_eq!(old.version, 1);
        assert_eq!(old.candidate.path, legacy);

        let unit = home.join("Library/Caches/ms-playwright/chromium-1248");
        fs::create_dir_all(&unit).unwrap();
        let m = fs::symlink_metadata(&unit).unwrap();
        let r = UnitRecord::directory(
            unit.clone(),
            "playwright-browsers",
            PLAN,
            (m.dev(), m.ino(), m.uid()),
            (384_000_000, 380_000_000),
            1_000_000,
        );
        r.validate(&home, &context.root).unwrap();
        let bytes = serde_json::to_vec(&r).unwrap();
        assert!(matches!(parse_record(&bytes).unwrap(), StoredRecord::Unit(u) if u == r));
        // A version-1 reader cannot mistake a unit record for a file record.
        assert!(serde_json::from_slice::<Record>(&bytes).is_err());
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["kind"] = "other".into();
        assert!(parse_record(&serde_json::to_vec(&value).unwrap()).is_err());
        value["kind"] = UNIT_RECORD_KIND.into();
        value["unexpected"] = true.into();
        assert!(parse_record(&serde_json::to_vec(&value).unwrap()).is_err());
        assert!(parse_record(br#"{"version":3}"#).is_err());
        assert!(parse_record(b"corrupt").is_err());
        for bad in [
            UnitRecord {
                provider: "large-old-files".into(),
                ..r.clone()
            },
            UnitRecord {
                plan_id: "../plan".into(),
                ..r.clone()
            },
            UnitRecord {
                original_path: home.join("Library/Caches/ms-playwright/../../Keychains"),
                ..r.clone()
            },
            UnitRecord {
                original_path: home.join(".optimac-rust-recovery/5001"),
                ..r.clone()
            },
            UnitRecord {
                original_path: home.join("Documents"),
                ..r.clone()
            },
        ] {
            assert!(bad.validate(&home, &context.root).is_err(), "{bad:?}");
        }
        drop(context);
        drop(cap);
        fs::remove_dir_all(home).unwrap();
    }
}
