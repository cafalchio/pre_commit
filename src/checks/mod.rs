use ratatui::style::Color;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Group
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Python,
    Rust,
    Ui,
    Integration,
    /// Checks that don't belong to a specific stack — always shown.
    All,
}

impl Group {
    pub const ALL: [Group; 5] = [Group::Python, Group::Rust, Group::Ui, Group::Integration, Group::All];

    pub fn label(self) -> &'static str {
        match self {
            Group::Python => "Python",
            Group::Rust => "Rust",
            Group::Ui => "UI",
            Group::Integration => "Integration",
            Group::All => "All",
        }
    }

    pub fn color(self) -> Color {
        match self {
            Group::Python => Color::Blue,
            Group::Rust => Color::Red,
            Group::Ui => Color::Magenta,
            Group::Integration => Color::Green,
            Group::All => Color::White,
        }
    }
}

// ---------------------------------------------------------------------------
// CheckDef
// ---------------------------------------------------------------------------

pub struct CheckDef {
    pub name: String,
    pub description: String,
    pub cmd: Vec<String>,
    pub only_when_staged: Option<String>,
    pub advisory: bool,
    pub group: Group,
    /// Checks sharing the same non-None `parallel_group` string run concurrently.
    pub parallel_group: Option<String>,
}

// ---------------------------------------------------------------------------
// JSON loading
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct CheckDefJson {
    name: String,
    description: String,
    cmd: Vec<String>,
    #[serde(default)]
    only_when_staged: Option<String>,
    #[serde(default)]
    advisory: bool,
    #[serde(default)]
    parallel_group: Option<String>,
}

#[derive(Deserialize)]
struct ChecksConfigJson {
    #[serde(default)]
    project_root: Option<String>,
    python: Vec<CheckDefJson>,
    rust: Vec<CheckDefJson>,
    ui: Vec<CheckDefJson>,
    integration: Vec<CheckDefJson>,
    all: Vec<CheckDefJson>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub struct ChecksConfig {
    pub project_root: Option<String>,
    pub checks: Vec<CheckDef>,
}

/// Resolve the path to `tests_config.json` at runtime.
///
/// The file must live alongside the runner binary.
/// During `cargo test`, `$CARGO_MANIFEST_DIR` is used as a fallback.
fn find_checks_config_path() -> Option<std::path::PathBuf> {
    // 1. Next to the binary
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let path = dir.join("tests_config.json");
            if path.is_file() {
                return Some(path);
            }
        }
    }

    // 2. Source tree fallback — set by cargo when running `cargo test`
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let path = std::path::PathBuf::from(manifest_dir)
            .join("src")
            .join("checks")
            .join("tests_config.json");
        if path.is_file() {
            return Some(path);
        }
    }

    None
}

/// Parse `tests_config.json` (loaded at runtime) into checks + project root.
pub fn load_checks_config() -> ChecksConfig {
    let path = find_checks_config_path()
        .expect("tests_config.json not found — place it alongside the binary");
    let json = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    let data: ChecksConfigJson =
        serde_json::from_str(&json).unwrap_or_else(|e| panic!("invalid {}: {e}", path.display()));

    let sections: [(Vec<CheckDefJson>, Group); 5] = [
        (data.python, Group::Python),
        (data.rust, Group::Rust),
        (data.ui, Group::Ui),
        (data.integration, Group::Integration),
        (data.all, Group::All),
    ];

    let mut checks = Vec::new();
    for (group_checks, group) in sections {
        for c in group_checks {
            checks.push(CheckDef {
                name: c.name,
                description: c.description,
                cmd: c.cmd,
                only_when_staged: c.only_when_staged,
                advisory: c.advisory,
                group,
                parallel_group: c.parallel_group,
            });
        }
    }

    ChecksConfig { project_root: data.project_root, checks }
}

/// Returns all checks in group order: Python → Rust → UI → Integration → All.
pub fn all_checks() -> Vec<CheckDef> {
    load_checks_config().checks
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn check_names_are_unique() {
        let checks = all_checks();
        let mut seen = HashSet::new();
        for c in &checks {
            assert!(seen.insert(c.name.as_str()), "duplicate check name: '{}'", c.name);
        }
    }

    #[test]
    fn checks_have_non_empty_cmd() {
        for c in all_checks() {
            assert!(!c.cmd.is_empty(), "check '{}' has empty cmd", c.name);
            assert!(!c.cmd[0].is_empty(), "check '{}' cmd[0] is blank", c.name);
        }
    }

    #[test]
    fn checks_have_name_and_description() {
        for c in all_checks() {
            assert!(!c.name.is_empty(), "a check has an empty name");
            assert!(!c.description.is_empty(), "check '{}' has empty description", c.name);
        }
    }

    #[test]
    fn group_labels_are_non_empty() {
        for g in Group::ALL {
            assert!(!g.label().is_empty(), "Group has empty label");
        }
    }

    #[test]
    fn every_group_has_at_least_one_check() {
        let checks = all_checks();
        for group in Group::ALL {
            let count = checks.iter().filter(|c| c.group == group).count();
            assert!(count > 0, "group '{}' has no checks", group.label());
        }
    }

    #[test]
    fn group_all_array_has_no_duplicates() {
        let mut seen = HashSet::new();
        for g in Group::ALL {
            assert!(seen.insert(g.label()), "duplicate group in Group::ALL: {}", g.label());
        }
    }
}
