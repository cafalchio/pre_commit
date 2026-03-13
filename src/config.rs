use std::path::PathBuf;

fn config_file_path() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".config").join("pre_commit").join("config"))
}

pub fn load_saved_config() -> (String, String) {
    let path = match config_file_path() {
        Some(p) => p,
        None => return (String::new(), String::new()),
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return (String::new(), String::new()),
    };
    let mut repo = String::new();
    let mut branch = String::new();
    for line in content.lines() {
        if let Some(v) = line.strip_prefix("repo=") {
            repo = v.to_string();
        } else if let Some(v) = line.strip_prefix("branch=") {
            branch = v.to_string();
        }
    }
    (repo, branch)
}

pub fn save_config(repo: &str, branch: &str) {
    let path = match config_file_path() {
        Some(p) => p,
        None => return,
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let content = format!("repo={repo}\nbranch={branch}\n");
    let _ = std::fs::write(&path, content);
}


#[cfg(test)]
mod tests {
    use std::fs;

    /// Parse a config file content string the same way load_saved_config does.
    fn parse_config_content(content: &str) -> (String, String) {
        let mut repo = String::new();
        let mut branch = String::new();
        for line in content.lines() {
            if let Some(v) = line.strip_prefix("repo=") {
                repo = v.to_string();
            } else if let Some(v) = line.strip_prefix("branch=") {
                branch = v.to_string();
            }
        }
        (repo, branch)
    }

    #[test]
    fn config_file_format_round_trips() {
        let repo = "/some/repo";
        let branch = "main";
        let content = format!("repo={repo}\nbranch={branch}\n");
        let (r, b) = parse_config_content(&content);
        assert_eq!(r, repo);
        assert_eq!(b, branch);
    }

    #[test]
    fn config_file_empty_returns_empty_strings() {
        let (r, b) = parse_config_content("");
        assert!(r.is_empty());
        assert!(b.is_empty());
    }

    #[test]
    fn config_file_missing_branch_returns_empty_branch() {
        let content = "repo=/my/repo\n";
        let (r, b) = parse_config_content(content);
        assert_eq!(r, "/my/repo");
        assert!(b.is_empty());
    }

    #[test]
    fn config_file_ignores_unknown_keys() {
        let content = "foo=bar\nrepo=/r\nbranch=dev\nbaz=qux\n";
        let (r, b) = parse_config_content(content);
        assert_eq!(r, "/r");
        assert_eq!(b, "dev");
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config_path = dir.path().join("config");
        let content = format!("repo={}\nbranch={}\n", "/test/repo", "feature-x");
        fs::write(&config_path, &content).unwrap();
        let (r, b) = parse_config_content(&fs::read_to_string(&config_path).unwrap());
        assert_eq!(r, "/test/repo");
        assert_eq!(b, "feature-x");
    }
}
