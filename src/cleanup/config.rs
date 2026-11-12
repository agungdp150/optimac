//! Strict schema-v2 overlay parsing. No configuration can authorize mutation.
use std::{
    collections::HashSet,
    io,
    path::{Component, Path, PathBuf},
};
use toml::Value;

#[derive(Debug, Clone)]
pub struct Config {
    pub value: Value,
}
fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
fn table<'a>(v: &'a Value, name: &str) -> io::Result<&'a toml::map::Map<String, Value>> {
    v.as_table()
        .ok_or_else(|| invalid(format!("{name}: expected table")))
}
fn fields(v: &Value, allowed: &[&str], name: &str) -> io::Result<()> {
    for key in table(v, name)?.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(invalid(format!("{name}: unknown field {key}")));
        }
    }
    Ok(())
}
fn choice(v: Option<&Value>, allowed: &[&str], key: &str) -> io::Result<()> {
    if let Some(v) = v
        && !v.as_str().is_some_and(|s| allowed.contains(&s))
    {
        return Err(invalid(format!("Invalid {key}")));
    }
    Ok(())
}
fn paths(v: Option<&Value>, key: &str) -> io::Result<()> {
    if let Some(v) = v {
        for path in v
            .as_array()
            .ok_or_else(|| invalid(format!("{key}: expected array")))?
        {
            let s = path
                .as_str()
                .ok_or_else(|| invalid("Expected path string"))?;
            if !(s.starts_with('/') || s.starts_with("~/"))
                || Path::new(s)
                    .components()
                    .any(|p| matches!(p, Component::ParentDir))
            {
                return Err(invalid(format!("Unsafe {key} path")));
            }
        }
    }
    Ok(())
}
impl Config {
    pub fn load(path: &Path) -> io::Result<Self> {
        use std::io::Read;
        let mut text = String::new();
        std::fs::File::open(path)?
            .take(1_048_577)
            .read_to_string(&mut text)?;
        if text.len() > 1_048_576 {
            return Err(invalid("Configuration exceeds 1 MiB"));
        }
        Self::parse(&text)
    }
    pub fn parse(text: &str) -> io::Result<Self> {
        let v: Value = toml::from_str(text).map_err(|e| invalid(e.to_string()))?;
        fields(
            &v,
            &[
                "schema_version",
                "defaults",
                "scope",
                "access",
                "recovery",
                "scan",
                "ui",
                "rule",
            ],
            "config",
        )?;
        if v.get("schema_version").and_then(Value::as_integer) != Some(2) {
            return Err(invalid("schema_version must be 2"));
        }
        let reference: Value = toml::from_str(include_str!("../../docs/rules.example.toml"))
            .map_err(|e| invalid(e.to_string()))?;
        for section in ["defaults", "scope", "access", "recovery", "scan", "ui"] {
            if let Some(actual) = v.get(section) {
                let template = table(&reference[section], section)?;
                fields(
                    actual,
                    &template.keys().map(String::as_str).collect::<Vec<_>>(),
                    section,
                )?;
                for (key, val) in table(actual, section)? {
                    let t = &template[key];
                    if val.type_str() != t.type_str() {
                        return Err(invalid(format!("{section}.{key}: incorrect type")));
                    }
                    if let Some(n) = val.as_integer()
                        && !(0..=1_000_000_000_000).contains(&n)
                    {
                        return Err(invalid(format!("Invalid {section}.{key}")));
                    }
                }
            }
        }
        if let Some(d) = v.get("defaults") {
            for key in ["auto_select", "allow_sudo", "follow_symlinks"] {
                if d.get(key).and_then(Value::as_bool) == Some(true) {
                    return Err(invalid(format!("{key} cannot be enabled")));
                }
            }
            for key in ["require_confirmation", "skip_cloud_and_network"] {
                if d.get(key).and_then(Value::as_bool) == Some(false) {
                    return Err(invalid(format!("{key} cannot be disabled")));
                }
            }
            choice(
                d.get("operation"),
                &["inventory", "recovery", "permanent", "guidance"],
                "operation",
            )?;
            choice(
                d.get("age_basis"),
                &["mtime_hint", "not_applicable"],
                "age_basis",
            )?;
            choice(
                d.get("size_basis"),
                &["aggregate_review_unit"],
                "size_basis",
            )?;
            choice(d.get("unknown_activity"), &["block"], "unknown_activity")?;
            choice(
                d.get("file_removal_default"),
                &["recovery"],
                "file_removal_default",
            )?;
        }
        if let Some(s) = v.get("scope") {
            for k in ["project_roots", "file_roots", "exclude_paths"] {
                paths(s.get(k), k)?;
            }
        }
        if let Some(a) = v.get("access") {
            choice(a.get("on_denied"), &["report_partial"], "on_denied")?;
        }
        if let Some(r) = v.get("recovery")
            && (r
                .get("auto_expire_days")
                .and_then(Value::as_integer)
                .is_some_and(|n| n != 0)
                || r.get("purge_requires_confirmation")
                    .and_then(Value::as_bool)
                    == Some(false))
        {
            return Err(invalid("Automatic/unconfirmed purge is unsupported"));
        }
        let mut ids = HashSet::new();
        if let Some(rules) = v.get("rule") {
            for r in rules
                .as_array()
                .ok_or_else(|| invalid("rule must be an array"))?
            {
                fields(
                    r,
                    &[
                        "id",
                        "provider",
                        "risk",
                        "enabled",
                        "operation",
                        "class",
                        "scope",
                        "scan_roots",
                        "exclude_paths",
                        "exclude_name_patterns",
                        "age_basis",
                        "min_age_days",
                        "min_size_bytes",
                        "high_risk",
                        "docker",
                    ],
                    "rule",
                )?;
                let id = r
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("Missing rule id"))?;
                if !ids.insert(id) {
                    return Err(invalid("Duplicate rule id"));
                }
                let builtin = reference["rule"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|x| x["id"].as_str() == Some(id))
                    .ok_or_else(|| invalid(format!("Unknown rule {id}")))?;
                for (k, val) in table(r, "rule")? {
                    if let Some(expected) = builtin.get(k).or_else(|| reference["defaults"].get(k))
                        && val.type_str() != expected.type_str()
                    {
                        return Err(invalid(format!("{id}.{k}: incorrect type")));
                    }
                }
                for k in ["provider", "class", "risk"] {
                    if let Some(val) = r.get(k)
                        && val != &builtin[k]
                    {
                        return Err(invalid(format!("{id}.{k} cannot override registry policy")));
                    }
                }
                choice(
                    r.get("operation"),
                    &["inventory", "recovery", "permanent", "guidance"],
                    "operation",
                )?;
                choice(r.get("scope"), &["provider", "projects", "files"], "scope")?;
                choice(
                    r.get("age_basis"),
                    &["mtime_hint", "not_applicable"],
                    "age_basis",
                )?;
                let operation = r
                    .get("operation")
                    .and_then(Value::as_str)
                    .unwrap_or("inventory");
                if builtin["risk"].as_str() == Some("protected")
                    && !matches!(operation, "inventory" | "guidance")
                {
                    return Err(invalid("Protected rules cannot mutate"));
                }
                if builtin["operation"].as_str() == Some("guidance")
                    && !matches!(operation, "inventory" | "guidance")
                {
                    return Err(invalid("Guidance-only provider cannot mutate"));
                }
                if operation == "permanent"
                    && matches!(
                        builtin["class"].as_str(),
                        Some("user_state" | "holding_area")
                    )
                    && r.get("high_risk") != Some(&Value::Boolean(true))
                {
                    return Err(invalid("Permanent user-state removal requires high_risk"));
                }
                for k in ["scan_roots", "exclude_paths"] {
                    paths(r.get(k), k)?;
                }
                for k in ["min_age_days", "min_size_bytes"] {
                    if let Some(n) = r.get(k)
                        && !n
                            .as_integer()
                            .is_some_and(|n| (0..=1_000_000_000_000).contains(&n))
                    {
                        return Err(invalid(format!("Invalid {k}")));
                    }
                }
                if r.get("age_basis").and_then(Value::as_str) == Some("not_applicable")
                    && r.get("min_age_days").and_then(Value::as_integer) != Some(0)
                {
                    return Err(invalid("not_applicable requires min_age_days = 0"));
                }
                if let Some(patterns) = r.get("exclude_name_patterns") {
                    for p in patterns
                        .as_array()
                        .ok_or_else(|| invalid("Expected pattern array"))?
                    {
                        let p = p
                            .as_str()
                            .ok_or_else(|| invalid("Expected pattern string"))?;
                        if p.contains("**") || p.contains(['/', '[', ']', '{', '}', '\\']) {
                            return Err(invalid("Unsupported basename pattern"));
                        }
                    }
                }
                if let Some(d) = r.get("docker") {
                    if !id.starts_with("docker-") {
                        return Err(invalid("Docker options on non-Docker rule"));
                    }
                    fields(d, &["context", "endpoint_policy", "allow_remote"], "docker")?;
                    if d.get("context").is_some_and(|v| {
                        !v.as_str()
                            .is_some_and(|s| !s.is_empty() && !s.chars().any(char::is_control))
                    }) {
                        return Err(invalid("Invalid Docker context"));
                    }
                    choice(
                        d.get("endpoint_policy"),
                        &["local_desktop_only"],
                        "endpoint_policy",
                    )?;
                    if d.get("allow_remote") != Some(&Value::Boolean(false)) {
                        return Err(invalid("Remote Docker mutation prohibited"));
                    }
                }
            }
        }
        for section in ["scan", "ui"] {
            if let Some(values) = v.get(section).and_then(Value::as_table) {
                for (key, value) in values {
                    if value.as_integer().is_some_and(|n| n == 0) {
                        return Err(invalid(format!("{section}.{key} must be positive")));
                    }
                }
            }
        }
        if let Some(defaults) = v.get("defaults")
            && defaults.get("age_basis").and_then(Value::as_str) == Some("not_applicable")
            && defaults.get("min_age_days").and_then(Value::as_integer) != Some(0)
        {
            return Err(invalid("not_applicable requires min_age_days = 0"));
        }
        Ok(Self { value: v })
    }
    pub fn scan_budget(&self, mut budget: super::Budget) -> io::Result<super::Budget> {
        let deep = budget.entries > 10_000;
        if let Some(scan) = self.value.get("scan") {
            let read = |key: &str, default: u64| -> io::Result<u64> {
                let n = scan
                    .get(key)
                    .and_then(Value::as_integer)
                    .map(|n| n as u64)
                    .unwrap_or(default);
                if n == 0 {
                    return Err(invalid(format!("{key} must be positive")));
                }
                Ok(n)
            };
            budget.entries = usize::try_from(read(
                if deep {
                    "deep_max_entries_per_provider"
                } else {
                    "quick_max_entries_per_provider"
                },
                budget.entries as u64,
            )?)
            .map_err(|_| invalid("Entry budget overflow"))?;
            budget.global_entries = usize::try_from(read(
                if deep {
                    "deep_max_entries_global"
                } else {
                    "quick_max_entries_global"
                },
                budget.global_entries as u64,
            )?)
            .map_err(|_| invalid("Global budget overflow"))?;
            budget.total = std::time::Duration::from_secs(read(
                if deep {
                    "deep_max_seconds_global"
                } else {
                    "quick_max_seconds_global"
                },
                budget.total.as_secs(),
            )?);
            budget.per_root = if deep {
                budget.total
            } else {
                std::time::Duration::from_secs(read(
                    "quick_max_seconds_per_provider",
                    budget.per_root.as_secs(),
                )?)
            };
        }
        Ok(budget)
    }
    pub fn expand_path(home: &Path, path: &str) -> PathBuf {
        path.strip_prefix("~/")
            .map(|p| home.join(p))
            .unwrap_or_else(|| path.into())
    }
}
pub(super) struct Job {
    pub id: String,
    pub roots: Vec<PathBuf>,
    pub unsupported: Option<String>,
    excludes: Vec<PathBuf>,
    patterns: Vec<String>,
}
impl Job {
    pub fn excluded(&self, path: &Path) -> bool {
        ((self.id == "app-user-caches"
            || self.id == "sandbox-app-caches"
            || self.id == "group-container-caches")
            && path.components().any(|c| {
                c.as_os_str().to_str().is_some_and(|n| {
                    n.starts_with("com.apple.") || n.starts_with("group.com.apple.")
                })
            }))
            || self.excludes.iter().any(|p| path.starts_with(p))
            || path.components().any(|c| {
                let Some(n) = c.as_os_str().to_str() else {
                    return true;
                };
                matches!(
                    n,
                    ".ssh"
                        | ".aws"
                        | ".gnupg"
                        | ".git"
                        | "Keychains"
                        | "CloudStorage"
                        | "Mobile Documents"
                        | "Mail"
                        | "Messages"
                        | "Notes"
                ) || n.starts_with(".env")
                    || n.ends_with(".photoslibrary")
                    || self.patterns.iter().any(|p| glob(p, n))
            })
    }
}
fn glob(pattern: &str, name: &str) -> bool {
    let p: Vec<_> = pattern.chars().collect();
    let n: Vec<_> = name.chars().collect();
    let (mut i, mut j, mut star, mut mark) = (0, 0, None, 0);
    while j < n.len() {
        if i < p.len() && (p[i] == '?' || p[i] == n[j]) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == '*' {
            star = Some(i);
            i += 1;
            mark = j;
        } else if let Some(s) = star {
            mark += 1;
            j = mark;
            i = s + 1;
        } else {
            return false;
        }
    }
    while i < p.len() && p[i] == '*' {
        i += 1;
    }
    i == p.len()
}
impl Config {
    pub(super) fn jobs(
        &self,
        home: &Path,
        pip: Option<&super::pip::Resolution>,
    ) -> io::Result<Vec<Job>> {
        self.jobs_with_env(home, |key| {
            if key == "PIP_CACHE_DIR" {
                pip.and_then(|p| p.path.as_ref().map(|v| v.as_os_str().to_owned()))
                    .or_else(|| std::env::var_os(key))
            } else {
                std::env::var_os(key)
            }
        })
    }
    pub(super) fn jobs_with_env(
        &self,
        home: &Path,
        env: impl Fn(&str) -> Option<std::ffi::OsString>,
    ) -> io::Result<Vec<Job>> {
        let expand = |v: Option<&Value>| -> Vec<PathBuf> {
            v.and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(|s| Self::expand_path(home, s))
                .collect()
        };
        let global = self.value.get("scope");
        let mut jobs = Vec::new();
        for r in self
            .value
            .get("rule")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let enabled = r
                .get("enabled")
                .or_else(|| self.value.get("defaults").and_then(|d| d.get("enabled")))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if !enabled {
                continue;
            }
            let id = r["id"].as_str().unwrap();
            let scope = r.get("scope").and_then(Value::as_str).unwrap_or("provider");
            let alias = super::registry::alias(id);
            let mut unsupported = None;
            let allowed = match scope {
                "files" => expand(global.and_then(|s| s.get("file_roots"))),
                "projects" => expand(global.and_then(|s| s.get("project_roots"))),
                _ => {
                    let root = super::registry::ROOTS
                        .iter()
                        .find(|(p, _)| *p == alias)
                        .and_then(
                            |(_, p)| match super::registry::resolve(home, alias, p, &env) {
                                Ok(root) => Some(root),
                                Err(message) => {
                                    unsupported = Some(message.into());
                                    None
                                }
                            },
                        )
                        .or_else(|| match id {
                            "app-user-caches" => Some(home.join("Library/Caches")),
                            "sandbox-app-caches" => Some(home.join("Library/Containers")),
                            "group-container-caches" => Some(home.join("Library/Group Containers")),
                            _ => None,
                        });
                    if root.is_none() && unsupported.is_none() {
                        unsupported =
                            Some("Provider discovery is not implemented; no scan performed".into());
                    }
                    root.into_iter().collect()
                }
            };
            let custom = expand(r.get("scan_roots"));
            if custom
                .iter()
                .any(|p| !allowed.iter().any(|a| p.starts_with(a)))
            {
                return Err(invalid(format!("{id}: scan root outside approved scope")));
            }
            // Scope types cannot be used to bypass provider protection.
            if scope != "provider"
                && !matches!(
                    (id, scope),
                    ("project-artifacts", "projects") | ("large-old-files" | "duplicates", "files")
                )
            {
                return Err(invalid(format!("{id}: unsupported scope type")));
            }
            let mut excludes = expand(global.and_then(|s| s.get("exclude_paths")));
            excludes.extend(expand(r.get("exclude_paths")));
            let patterns = r
                .get("exclude_name_patterns")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
            jobs.push(Job {
                id: id.into(),
                roots: if custom.is_empty() { allowed } else { custom },
                unsupported,
                excludes,
                patterns,
            });
        }
        Ok(jobs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn example_and_invalid_overlays() {
        assert!(Config::parse(include_str!("../../docs/rules.example.toml")).is_ok());
        for input in [
            "schema_version=1",
            "schema_version=2\nunknown=true",
            "schema_version=2\n[defaults]\nallow_sudo=true",
            "schema_version=2\n[scope]\nfile_roots=['/tmp/../etc']",
            "schema_version=2\n[[rule]]\nid='unknown'",
        ] {
            assert!(Config::parse(input).is_err(), "{input}");
        }
    }
}
