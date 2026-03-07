use std::path::PathBuf;

pub struct Config {
    pub repo: PathBuf,
    pub venv: Option<PathBuf>,
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

/// Load saved (repo, venv) strings from the config file.
/// Returns empty strings if the file doesn't exist or can't be read.
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
    let mut venv = String::new();
    for line in content.lines() {
        if let Some(v) = line.strip_prefix("repo=") {
            repo = v.to_string();
        } else if let Some(v) = line.strip_prefix("venv=") {
            venv = v.to_string();
        }
    }
    (repo, venv)
}

/// Persist (repo, venv) to the config file, creating the directory if needed.
pub fn save_config(repo: &str, venv: &str) {
    let path = match config_file_path() {
        Some(p) => p,
        None => return,
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let content = format!("repo={repo}\nvenv={venv}\n");
    let _ = std::fs::write(&path, content);
}

// ---------------------------------------------------------------------------
// CLI argument parsing
// ---------------------------------------------------------------------------

pub fn parse_args() -> Result<Config, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut repo: Option<PathBuf> = None;
    let mut venv: Option<PathBuf> = None;
    let mut run_check: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--repo" => {
                i += 1;
                let p = args.get(i).ok_or("--repo requires a path")?;
                repo = Some(PathBuf::from(p));
            }
            "--venv" => {
                i += 1;
                let p = args.get(i).ok_or("--venv requires a path")?;
                venv = Some(PathBuf::from(p));
            }
            "--check" => {
                i += 1;
                let name = args.get(i).ok_or("--check requires a check name")?;
                run_check = Some(name.clone());
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: pre_commit [--repo <path>] [--venv <path>] [--check <name>]\n\
                     \n\
                     Options:\n\
                       --repo  <path>  Absolute path to the mcp-context-forge repo\n\
                                       (default: current directory)\n\
                       --venv  <path>  Python virtual environment; <venv>/bin prepended to PATH\n\
                       --check <name>  Run a single named check in headless mode and exit\n\
                       -h, --help      Show this help\n"
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
    if let Some(ref v) = venv {
        if !v.is_dir() {
            return Err(format!("venv path does not exist: {}", v.display()));
        }
    }
    Ok(Config { repo, venv, run_check })
}
