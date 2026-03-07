use std::path::PathBuf;

pub struct Config {
    pub repo: PathBuf,
    pub branch: Option<String>,
    pub run_check: Option<String>,
}

// ---------------------------------------------------------------------------
// Persistent config file  (~/.config/pre_commit/config)
// ---------------------------------------------------------------------------

fn config_file_path() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".config").join("pre_commit").join("config"))
}

/// Load saved (repo, branch) strings from the config file.
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

/// Persist (repo, branch) to the config file, creating the directory if needed.
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

// ---------------------------------------------------------------------------
// CLI argument parsing
// ---------------------------------------------------------------------------

pub fn parse_args() -> Result<Config, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut repo: Option<PathBuf> = None;
    let mut branch: Option<String> = None;
    let mut run_check: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--repo" => {
                i += 1;
                let p = args.get(i).ok_or("--repo requires a path")?;
                repo = Some(PathBuf::from(p));
            }
            "--branch" => {
                i += 1;
                let b = args.get(i).ok_or("--branch requires a branch name or PR number")?;
                branch = Some(b.clone());
            }
            "--check" => {
                i += 1;
                let name = args.get(i).ok_or("--check requires a check name")?;
                run_check = Some(name.clone());
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: pre_commit [--repo <path>] [--branch <branch|pr>] [--check <name>]\n\
                     \n\
                     Options:\n\
                       --repo   <path>      Absolute path to the repo root (default: cwd)\n\
                       --branch <branch|pr> Branch name or PR number to checkout before running\n\
                       --check  <name>      Run a single named check in headless mode and exit\n\
                       -h, --help           Show this help\n"
                );
                std::process::exit(0);
            }
            arg if !arg.starts_with('-') && repo.is_none() => {
                repo = Some(PathBuf::from(arg));
            }
            arg => return Err(format!("unknown argument: {arg}")),
        }
        i += 1;
    }
    let repo = repo.unwrap_or_else(|| std::env::current_dir().expect("cwd"));
    if !repo.is_dir() {
        return Err(format!("repo path does not exist: {}", repo.display()));
    }
    Ok(Config { repo, branch, run_check })
}
