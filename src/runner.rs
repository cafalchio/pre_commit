use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;

use crate::checks::all_checks;

/// Run a single named check in headless (non-TUI) mode and exit.
/// All output goes to stdout; exits with code 1 on failure, 2 on error.
pub fn run_headless(check_name: &str, repo: &PathBuf) {
    let checks = all_checks();
    let check = checks.iter().find(|c| c.name == check_name).unwrap_or_else(|| {
        eprintln!("error: unknown check '{check_name}'");
        eprintln!("Available checks:");
        for c in &checks {
            eprintln!("  {}", c.name);
        }
        std::process::exit(2);
    });

    println!("==> {} : {}", check.name, check.description);
    println!("    cmd : {}", check.cmd.join(" "));
    println!("    repo: {}", repo.display());
    println!();

    let (prog, args) = check.cmd.split_first().expect("empty cmd");
    let start = std::time::Instant::now();

    let mut child = Command::new(prog)
        .args(args)
        .current_dir(repo)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| {
            println!("spawn error: {e}");
            std::process::exit(2);
        });

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let out_thread = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            println!("{line}");
        }
    });
    let err_thread = thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            println!("{line}");
        }
    });

    let status = child.wait().unwrap_or_else(|e| {
        println!("wait error: {e}");
        std::process::exit(2);
    });
    out_thread.join().ok();
    err_thread.join().ok();

    let elapsed = start.elapsed().as_secs_f64();
    if status.success() {
        println!("\nOK  {} ({:.1}s)", check.name, elapsed);
    } else {
        println!("\nFAIL {} ({:.1}s)", check.name, elapsed);
        use std::io::Write;
        let _ = std::io::stdout().flush();
        std::process::exit(1);
    }
}
