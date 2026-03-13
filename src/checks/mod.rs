use ratatui::style::Color;
use serde::Deserialize;

#[derive(Clone)]
pub struct GroupDef {
    pub label: String,
    pub color: Color,
}

pub struct CheckDef {
    pub name: String,
    pub description: String,
    pub cmd: Vec<String>,
    pub group_idx: usize,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum CmdInput {
    Single(String),
    Multi(Vec<String>),
}

impl CmdInput {
    fn into_vec(self) -> Vec<String> {
        match self {
            CmdInput::Single(s) => s.split_whitespace().map(str::to_string).collect(),
            CmdInput::Multi(v) => v,
        }
    }
}

#[derive(Deserialize)]
struct CheckDefJson {
    name: String,
    description: String,
    cmd: CmdInput,
}

#[derive(Deserialize)]
struct GroupJson {
    label: String,
    color: String,
    checks: Vec<CheckDefJson>,
}

#[derive(Deserialize)]
struct ChecksConfigJson {
    #[serde(default)]
    project_root: Option<String>,
    #[serde(default)]
    path_log_file: Option<String>,
    groups: Vec<GroupJson>,
}

pub struct ChecksConfig {
    pub project_root: Option<String>,
    pub path_log_file: Option<String>,
    pub groups: Vec<GroupDef>,
    pub checks: Vec<CheckDef>,
}

fn parse_color(s: &str) -> Color {
    match s.to_lowercase().as_str() {
        "blue" => Color::Blue,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        _ => Color::White,
    }
}

fn find_checks_config_path() -> Option<std::path::PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let path = dir.join("tests_config.json");
            if path.is_file() {
                return Some(path);
            }
        }
    }

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

fn fatal(msg: &str) -> ! {
    eprintln!("\n  Error: {msg}\n");
    std::process::exit(1);
}

pub fn load_checks_config() -> ChecksConfig {
    let path = find_checks_config_path().unwrap_or_else(|| {
        fatal("tests_config.json not found — place it alongside the binary");
    });
    let json = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        fatal(&format!("cannot read {}: {e}", path.display()));
    });
    let data: ChecksConfigJson = serde_json::from_str(&json).unwrap_or_else(|e| {
        fatal(&format!("invalid JSON in {}: {e}", path.display()));
    });

    let mut groups = Vec::new();
    let mut checks = Vec::new();
    for (group_idx, group_json) in data.groups.into_iter().enumerate() {
        groups.push(GroupDef {
            label: group_json.label,
            color: parse_color(&group_json.color),
        });
        for c in group_json.checks {
            checks.push(CheckDef {
                name: c.name,
                description: c.description,
                cmd: c.cmd.into_vec(),
                group_idx,
            });
        }
    }

    ChecksConfig {
        project_root: data.project_root,
        path_log_file: data.path_log_file,
        groups,
        checks,
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn check_names_are_unique() {
        let checks = load_checks_config().checks;
        let mut seen = HashSet::new();
        for c in &checks {
            assert!(seen.insert(c.name.as_str()), "duplicate check name: '{}'", c.name);
        }
    }

    #[test]
    fn checks_have_non_empty_cmd() {
        for c in load_checks_config().checks {
            assert!(!c.cmd.is_empty(), "check '{}' has empty cmd", c.name);
            assert!(!c.cmd[0].is_empty(), "check '{}' cmd[0] is blank", c.name);
        }
    }

    #[test]
    fn checks_have_name_and_description() {
        for c in load_checks_config().checks {
            assert!(!c.name.is_empty(), "a check has an empty name");
            assert!(!c.description.is_empty(), "check '{}' has empty description", c.name);
        }
    }

    #[test]
    fn groups_have_non_empty_labels() {
        for g in load_checks_config().groups {
            assert!(!g.label.is_empty(), "a group has an empty label");
        }
    }

    #[test]
    fn every_group_has_at_least_one_check() {
        let config = load_checks_config();
        for (idx, g) in config.groups.iter().enumerate() {
            let count = config.checks.iter().filter(|c| c.group_idx == idx).count();
            assert!(count > 0, "group '{}' has no checks", g.label);
        }
    }

    #[test]
    fn all_group_labels_are_unique() {
        let mut seen = HashSet::new();
        for g in load_checks_config().groups {
            assert!(seen.insert(g.label.clone()), "duplicate group label: {}", g.label);
        }
    }
}
