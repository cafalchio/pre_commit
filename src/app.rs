use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use ratatui::layout::Rect;
use ratatui::widgets::ListState;

use crate::checks::{all_checks, CheckDef, Group};
use crate::config::{save_config, Config};

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub enum CheckStatus {
    Pending,
    Running,
    Passed(f64),
    Failed(f64),
    Skipped,
    Advisory(f64),
}

#[derive(Clone)]
pub enum Mode {
    Setup,
    Selecting,
    Running { idx: usize },
    Done,
}

#[derive(Clone)]
pub enum ListEntry {
    GroupHeader(Group),
    Check(usize),
}

pub enum RunMsg {
    Line(String),
    Done { success: bool, elapsed: f64 },
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

pub struct App {
    pub checks: Vec<CheckDef>,
    pub entries: Vec<ListEntry>,
    pub selected: Vec<bool>,
    pub statuses: Vec<CheckStatus>,
    pub list_state: ListState,
    pub output_lines: Vec<String>,
    pub output_scroll: usize,
    pub mode: Mode,
    pub staged_files: Vec<String>,
    pub list_area: Option<Rect>,
    pub run_rx: Option<mpsc::Receiver<RunMsg>>,
    pub cancel_flag: Option<Arc<AtomicBool>>,
    pub repo_root: PathBuf,
    // Setup form
    pub setup_repo: String,
    pub setup_branch: String,  // branch name or PR number
    pub setup_focus: usize,
    pub setup_error: Option<String>,
    pub setup_log: Vec<String>, // checkout output shown on error
    pub current_branch: String, // displayed in setup form
}

impl App {
    pub fn new(config: Config) -> Self {
        let checks = all_checks();
        let n = checks.len();
        let entries = build_entries(&checks);
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        let (saved_repo, saved_branch) = crate::config::load_saved_config();
        let setup_repo = if config.repo != std::env::current_dir().unwrap_or_default() {
            config.repo.to_string_lossy().to_string()
        } else if !saved_repo.is_empty() {
            saved_repo
        } else {
            config.repo.to_string_lossy().to_string()
        };
        let setup_branch = config.branch.unwrap_or(saved_branch);

        let current_branch = detect_current_branch(&PathBuf::from(&setup_repo));

        App {
            selected: vec![true; n],
            statuses: vec![CheckStatus::Pending; n],
            entries,
            checks,
            list_state,
            output_lines: Vec::new(),
            output_scroll: 0,
            mode: Mode::Setup,
            staged_files: Vec::new(),
            list_area: None,
            run_rx: None,
            cancel_flag: None,
            repo_root: config.repo,
            setup_repo,
            setup_branch,
            setup_focus: 0,
            setup_error: None,
            setup_log: Vec::new(),
            current_branch,
        }
    }

    // ── Setup ────────────────────────────────────────────────────────────────

    pub fn confirm_setup(&mut self) {
        let repo = PathBuf::from(self.setup_repo.trim());
        if !repo.is_dir() {
            self.setup_error = Some(format!("path does not exist: {}", repo.display()));
            self.setup_log.clear();
            return;
        }

        // Branch / PR checkout
        let branch_input = self.setup_branch.trim().to_string();
        if !branch_input.is_empty() {
            match checkout_branch_or_pr(&repo, &branch_input) {
                Ok(log) => {
                    self.setup_log = log;
                    self.current_branch = detect_current_branch(&repo);
                }
                Err((log, msg)) => {
                    self.setup_error = Some(msg);
                    self.setup_log = log;
                    return;
                }
            }
        } else {
            self.current_branch = detect_current_branch(&repo);
        }

        save_config(self.setup_repo.trim(), self.setup_branch.trim());

        self.repo_root = repo;
        self.staged_files = get_staged_files(&self.repo_root);
        self.setup_error = None;
        self.mode = Mode::Selecting;
    }

    pub fn setup_type_char(&mut self, c: char) {
        match self.setup_focus {
            0 => self.setup_repo.push(c),
            _ => self.setup_branch.push(c),
        }
        self.setup_error = None;
        self.setup_log.clear();
        // Update current_branch preview as repo path is typed
        if self.setup_focus == 0 {
            self.current_branch = detect_current_branch(&PathBuf::from(self.setup_repo.trim()));
        }
    }

    pub fn setup_backspace(&mut self) {
        match self.setup_focus {
            0 => { self.setup_repo.pop(); }
            _ => { self.setup_branch.pop(); }
        }
        self.setup_error = None;
        self.setup_log.clear();
        if self.setup_focus == 0 {
            self.current_branch = detect_current_branch(&PathBuf::from(self.setup_repo.trim()));
        }
    }

    // ── Group helpers ────────────────────────────────────────────────────────

    pub fn group_state(&self, group: Group) -> (usize, usize) {
        self.checks
            .iter()
            .enumerate()
            .filter(|(_, c)| c.group == group)
            .fold((0, 0), |(sel, tot), (i, _)| {
                (sel + self.selected[i] as usize, tot + 1)
            })
    }

    pub fn toggle_group(&mut self, group: Group) {
        let (sel, tot) = self.group_state(group);
        let new_val = sel < tot;
        for (i, c) in self.checks.iter().enumerate() {
            if c.group == group {
                self.selected[i] = new_val;
            }
        }
    }

    // ── Navigation & selection ───────────────────────────────────────────────

    pub fn toggle_current(&mut self) {
        if let Some(ei) = self.list_state.selected() {
            match self.entries[ei].clone() {
                ListEntry::GroupHeader(g) => self.toggle_group(g),
                ListEntry::Check(ci) => self.selected[ci] = !self.selected[ci],
            }
        }
    }

    pub fn select_all(&mut self) {
        self.selected.iter_mut().for_each(|s| *s = true);
    }

    pub fn select_none(&mut self) {
        self.selected.iter_mut().for_each(|s| *s = false);
    }

    pub fn move_up(&mut self) {
        let i = self.list_state.selected().unwrap_or(0);
        if i > 0 {
            self.list_state.select(Some(i - 1));
        }
    }

    pub fn move_down(&mut self) {
        let i = self.list_state.selected().unwrap_or(0);
        if i + 1 < self.entries.len() {
            self.list_state.select(Some(i + 1));
        }
    }

    pub fn handle_mouse_click(&mut self, col: u16, row: u16) {
        if let Some(area) = self.list_area {
            if col >= area.x
                && col < area.x + area.width
                && row >= area.y
                && row < area.y + area.height
            {
                let inner_row = row.saturating_sub(area.y + 1) as usize;
                let entry_idx = self.list_state.offset() + inner_row;
                if entry_idx < self.entries.len() {
                    self.list_state.select(Some(entry_idx));
                    self.toggle_current();
                }
            }
        }
    }

    pub fn output_scroll_up(&mut self) {
        self.output_scroll = self.output_scroll.saturating_sub(3);
    }

    pub fn output_scroll_down(&mut self) {
        let max = self.output_lines.len().saturating_sub(1);
        self.output_scroll = (self.output_scroll + 3).min(max);
    }

    // ── Running ──────────────────────────────────────────────────────────────

    /// Signal the running subprocess to terminate and clean up channels.
    pub fn cancel_running(&mut self) {
        if let Some(flag) = self.cancel_flag.take() {
            flag.store(true, Ordering::Relaxed);
        }
        self.run_rx = None;
    }

    /// Go back to the selection screen, resetting all statuses and output.
    pub fn reset_to_selecting(&mut self) {
        self.cancel_running();
        for s in &mut self.statuses {
            *s = CheckStatus::Pending;
        }
        self.output_lines.clear();
        self.output_scroll = 0;
        self.mode = Mode::Selecting;
    }

    pub fn start_running(&mut self) {
        for s in &mut self.statuses {
            *s = CheckStatus::Pending;
        }
        self.output_lines.clear();
        self.output_lines.push(format!(
            "Branch: {}  |  Staged: {}",
            self.current_branch,
            if self.staged_files.is_empty() {
                "(none)".to_string()
            } else {
                self.staged_files.join(", ")
            }
        ));
        self.output_lines.push(String::new());
        self.output_scroll = 0;
        self.run_rx = None;
        self.mode = Mode::Running { idx: 0 };
    }

    /// Called every frame while Running.
    pub fn tick_running(&mut self) {
        loop {
            let idx = match &self.mode {
                Mode::Running { idx } => *idx,
                _ => return,
            };
            if idx >= self.checks.len() {
                self.mode = Mode::Done;
                self.run_rx = None;
                return;
            }
            if self.run_rx.is_none() {
                self.spawn_or_skip(idx);
                if self.run_rx.is_none() {
                    continue;
                }
            }
            self.poll_rx(idx);
            return;
        }
    }

    fn spawn_or_skip(&mut self, idx: usize) {
        let selected = self.selected[idx];
        let check_name = self.checks[idx].name;
        let cmd: Vec<&'static str> = self.checks[idx].cmd.clone();

        if !selected {
            self.statuses[idx] = CheckStatus::Skipped;
            self.output_lines.push(format!("[-] SKIP  {check_name}  (not selected)"));
            self.output_lines.push(String::new());
            self.advance(idx);
            return;
        }

        self.statuses[idx] = CheckStatus::Running;
        self.output_lines.push(format!("┌─ Running: {check_name} "));
        self.output_scroll = self.output_lines.len().saturating_sub(1);

        let (tx, rx) = mpsc::channel::<RunMsg>();
        self.run_rx = Some(rx);

        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel_flag = Some(Arc::clone(&cancel));

        let repo_root = self.repo_root.clone();

        thread::spawn(move || {
            let (prog, args) = cmd.split_first().expect("empty cmd");
            let start = Instant::now();

            let mut command = Command::new(prog);
            command
                .args(args)
                .current_dir(&repo_root)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            let mut child = match command.spawn() {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(RunMsg::Line(format!("spawn error: {e}")));
                    let _ = tx.send(RunMsg::Done { success: false, elapsed: 0.0 });
                    return;
                }
            };

            let stdout = child.stdout.take().unwrap();
            let stderr = child.stderr.take().unwrap();

            let tx_out = tx.clone();
            let out_thread = thread::spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    let _ = tx_out.send(RunMsg::Line(line));
                }
            });
            let tx_err = tx.clone();
            let err_thread = thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    let _ = tx_err.send(RunMsg::Line(line));
                }
            });

            // Poll for completion or cancellation every 50ms.
            let status = loop {
                if cancel.load(Ordering::Relaxed) {
                    child.kill().ok();
                    child.wait().ok(); // reap zombie
                    break None;        // cancelled → report as failed
                }
                match child.try_wait() {
                    Ok(Some(s)) => break Some(s),
                    Ok(None) => thread::sleep(Duration::from_millis(50)),
                    Err(_) => break None,
                }
            };

            out_thread.join().ok();
            err_thread.join().ok();

            let elapsed = start.elapsed().as_secs_f64();
            let success = status.map(|s| s.success()).unwrap_or(false);
            let _ = tx.send(RunMsg::Done { success, elapsed });
        });
    }

    fn poll_rx(&mut self, idx: usize) {
        let messages: Vec<RunMsg> = {
            let Some(rx) = &self.run_rx else { return };
            let mut buf = Vec::new();
            loop {
                match rx.try_recv() {
                    Ok(msg) => buf.push(msg),
                    Err(_) => break,
                }
            }
            buf
        };

        let mut done: Option<(bool, f64)> = None;
        for msg in messages {
            match msg {
                RunMsg::Line(line) => {
                    self.output_lines.push(format!("│ {line}"));
                    self.output_scroll = self.output_lines.len().saturating_sub(1);
                }
                RunMsg::Done { success, elapsed } => done = Some((success, elapsed)),
            }
        }

        if let Some((success, elapsed)) = done {
            self.run_rx = None;
            let check_name = self.checks[idx].name;
            let check_advisory = self.checks[idx].advisory;
            self.statuses[idx] = if success {
                self.output_lines.push(format!("└─ [+] OK    {check_name}  ({elapsed:.1}s)"));
                CheckStatus::Passed(elapsed)
            } else if check_advisory {
                self.output_lines.push(format!("└─ [!] WARN  {check_name}  ({elapsed:.1}s)"));
                CheckStatus::Advisory(elapsed)
            } else {
                self.output_lines.push(format!("└─ [x] FAIL  {check_name}  ({elapsed:.1}s)"));
                CheckStatus::Failed(elapsed)
            };
            self.output_lines.push(String::new());
            self.output_scroll = self.output_lines.len().saturating_sub(1);
            self.advance(idx);
        }
    }

    fn advance(&mut self, idx: usize) {
        let next = idx + 1;
        self.mode = if next >= self.checks.len() {
            Mode::Done
        } else {
            Mode::Running { idx: next }
        };
    }

    pub fn summary_counts(&self) -> (usize, usize, usize, usize) {
        let (mut passed, mut failed, mut skipped, mut advisory) = (0, 0, 0, 0);
        for s in &self.statuses {
            match s {
                CheckStatus::Passed(_) => passed += 1,
                CheckStatus::Failed(_) => failed += 1,
                CheckStatus::Skipped => skipped += 1,
                CheckStatus::Advisory(_) => advisory += 1,
                _ => {}
            }
        }
        (passed, failed, skipped, advisory)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn build_entries(checks: &[CheckDef]) -> Vec<ListEntry> {
    let mut entries = Vec::new();
    for group in Group::ALL {
        entries.push(ListEntry::GroupHeader(group));
        for (i, c) in checks.iter().enumerate() {
            if c.group == group {
                entries.push(ListEntry::Check(i));
            }
        }
    }
    entries
}

pub fn get_staged_files(repo: &PathBuf) -> Vec<String> {
    Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=ACMR"])
        .current_dir(repo)
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Returns the current git branch name, or "?" if it can't be determined.
pub fn detect_current_branch(repo: &PathBuf) -> String {
    if !repo.is_dir() {
        return String::from("?");
    }
    Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "?".to_string())
}

/// Checkout a branch name or PR number.
/// Returns `Ok(log_lines)` on success or `Err((log_lines, error_msg))` on failure.
pub fn checkout_branch_or_pr(
    repo: &PathBuf,
    branch_or_pr: &str,
) -> Result<Vec<String>, (Vec<String>, String)> {
    // If already on this branch (and it's not a PR number), skip.
    let is_pr = branch_or_pr.chars().all(|c| c.is_ascii_digit());
    if !is_pr {
        let current = detect_current_branch(repo);
        if current == branch_or_pr {
            return Ok(vec![format!("Already on branch '{branch_or_pr}'")]);
        }
    }

    let (prog, args): (&str, Vec<&str>) = if is_pr {
        ("gh", vec!["pr", "checkout", branch_or_pr])
    } else {
        ("git", vec!["checkout", branch_or_pr])
    };

    let output = Command::new(prog)
        .args(&args)
        .current_dir(repo)
        .output()
        .map_err(|e| (vec![], format!("failed to run '{prog}': {e}")))?;

    let log: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .chain(String::from_utf8_lossy(&output.stderr).lines())
        .map(str::to_string)
        .filter(|l| !l.is_empty())
        .collect();

    if output.status.success() {
        Ok(log)
    } else {
        let code = output.status.code().unwrap_or(-1);
        Err((log, format!("'{prog} checkout' failed (exit {code})")))
    }
}
