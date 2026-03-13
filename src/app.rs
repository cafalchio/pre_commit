use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use ratatui::layout::Rect;
use ratatui::widgets::ListState;
use sysinfo::System;

use crate::checks::{load_checks_config, CheckDef, GroupDef};
use crate::config::save_config;

#[derive(Clone)]
pub enum CheckStatus {
    Pending,
    Running,
    Passed(f64),
    Failed(f64),
    Skipped,
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
    GroupHeader(usize),
    Check(usize),
}

pub enum RunMsg {
    Line { line: String },
    Done { idx: usize, success: bool, elapsed: f64 },
}

pub struct App {
    pub groups: Vec<GroupDef>,
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
    cancel_flag: Option<Arc<AtomicBool>>,
    log_file: Option<std::fs::File>,
    failed_log_file: Option<std::fs::File>,
    current_check_buf: Vec<String>,
    pub repo_root: PathBuf,
    log_dir: Option<PathBuf>,
    pub mouse_capture: bool,
    pub setup_repo: String,
    pub setup_branch: String,
    pub setup_focus: usize,
    pub setup_error: Option<String>,
    pub setup_log: Vec<String>,
    pub current_branch: String,
    pub cpu_pct: f32,
    pub mem_pct: f32,
    sys: System,
}

impl App {
    pub fn new() -> Self {
        let checks_config = load_checks_config();
        let groups = checks_config.groups;
        let checks = checks_config.checks;
        let n = checks.len();
        let entries = build_entries(&checks, &groups);
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        let (saved_repo, saved_branch) = crate::config::load_saved_config();
        let setup_repo = if !saved_repo.is_empty() {
            saved_repo
        } else if let Some(root) = checks_config.project_root {
            root
        } else {
            std::env::current_dir().unwrap_or_default().to_string_lossy().to_string()
        };
        let setup_branch = saved_branch;
        let log_dir = checks_config.path_log_file.map(PathBuf::from).or_else(|| {
            std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()))
        });

        let current_branch = detect_current_branch(&PathBuf::from(&setup_repo));

        let mut sys = System::new();
        sys.refresh_cpu_usage();
        sys.refresh_memory();

        App {
            groups,
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
            log_file: None,
            failed_log_file: None,
            current_check_buf: Vec::new(),
            repo_root: PathBuf::from(&setup_repo),
            log_dir,
            mouse_capture: true,
            setup_repo,
            setup_branch,
            setup_focus: 0,
            setup_error: None,
            setup_log: Vec::new(),
            current_branch,
            cpu_pct: 0.0,
            mem_pct: 0.0,
            sys,
        }
    }

    pub fn refresh_sys_stats(&mut self) {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.cpu_pct = self.sys.global_cpu_usage();
        let total = self.sys.total_memory();
        if total > 0 {
            self.mem_pct = self.sys.used_memory() as f32 / total as f32 * 100.0;
        }
    }

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

    pub fn group_state(&self, group_idx: usize) -> (usize, usize) {
        self.checks
            .iter()
            .enumerate()
            .filter(|(_, c)| c.group_idx == group_idx)
            .fold((0, 0), |(sel, tot), (i, _)| {
                (sel + self.selected[i] as usize, tot + 1)
            })
    }

    pub fn toggle_group(&mut self, group_idx: usize) {
        let (sel, tot) = self.group_state(group_idx);
        let new_val = sel < tot;
        for (i, c) in self.checks.iter().enumerate() {
            if c.group_idx == group_idx {
                self.selected[i] = new_val;
            }
        }
    }

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

    pub fn cancel_running(&mut self) {
        if let Some(flag) = self.cancel_flag.take() {
            flag.store(true, Ordering::Relaxed);
        }
        self.run_rx = None;
        if let Some(ref mut f) = self.log_file {
            let _ = writeln!(f, "==> Cancelled");
        }
        self.log_file = None;
        self.failed_log_file = None;
        self.current_check_buf.clear();
    }

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
        self.cancel_running();
        for s in &mut self.statuses {
            *s = CheckStatus::Pending;
        }
        self.output_lines.clear();
        let header = format!(
            "Branch: {}  |  Staged: {}",
            self.current_branch,
            if self.staged_files.is_empty() {
                "(none)".to_string()
            } else {
                self.staged_files.join(", ")
            }
        );
        self.output_lines.push(header.clone());
        self.output_lines.push(String::new());
        self.output_scroll = 0;
        self.run_rx = None;

        let log_dir = self.log_dir.clone().unwrap_or_else(|| self.repo_root.clone());
        let log_path = log_dir.join("last_run.log");
        self.log_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&log_path)
            .ok();
        if let Some(ref mut f) = self.log_file {
            let _ = writeln!(f, "{header}");
            let _ = writeln!(f);
        }
        let failed_log_path = log_dir.join("last_failed.log");
        self.failed_log_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&failed_log_path)
            .ok();
        if let Some(ref mut f) = self.failed_log_file {
            let _ = writeln!(f, "{header}");
            let _ = writeln!(f);
        }
        self.current_check_buf.clear();

        self.mode = Mode::Running { idx: 0 };
    }

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
                self.spawn_check(idx);
                if self.run_rx.is_none() {
                    continue;
                }
            }
            self.poll_check();
            return;
        }
    }

    fn spawn_check(&mut self, idx: usize) {
        if !self.selected[idx] {
            self.statuses[idx] = CheckStatus::Skipped;
            self.output_lines.push(String::new());
            self.advance_to(idx + 1);
            return;
        }

        let check_name = self.checks[idx].name.clone();
        let cmd = self.checks[idx].cmd.clone();

        self.statuses[idx] = CheckStatus::Running;
        let run_line = format!("┌─ Running: {check_name} ");
        if let Some(ref mut f) = self.log_file {
            let _ = writeln!(f, "{run_line}");
        }
        self.current_check_buf.clear();
        self.current_check_buf.push(run_line.clone());
        self.output_lines.push(run_line);

        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel_flag = Some(Arc::clone(&cancel));

        let repo_root = self.repo_root.clone();
        let (tx, rx) = mpsc::channel::<RunMsg>();
        self.run_rx = Some(rx);

        thread::spawn(move || {
            let cmd = if cmd.len() == 1 {
                vec!["bash".to_string(), "-c".to_string(), cmd.into_iter().next().unwrap()]
            } else {
                cmd
            };
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
                    let _ = tx.send(RunMsg::Line { line: format!("spawn error: {e}") });
                    let _ = tx.send(RunMsg::Done { idx, success: false, elapsed: 0.0 });
                    return;
                }
            };

            let stdout = child.stdout.take().unwrap();
            let stderr = child.stderr.take().unwrap();

            let tx_out = tx.clone();
            let out_thread = thread::spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    let _ = tx_out.send(RunMsg::Line { line });
                }
            });
            let tx_err = tx.clone();
            let err_thread = thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    let _ = tx_err.send(RunMsg::Line { line });
                }
            });

            let status = loop {
                if cancel.load(Ordering::Relaxed) {
                    child.kill().ok();
                    child.wait().ok();
                    break None;
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
            let _ = tx.send(RunMsg::Done { idx, success, elapsed });
        });

        self.output_scroll = self.output_lines.len().saturating_sub(1);
    }

    fn poll_check(&mut self) {
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

        let mut next_idx = None;
        for msg in messages {
            match msg {
                RunMsg::Line { line, .. } => {
                    let formatted = format!("│ {line}");
                    if let Some(ref mut f) = self.log_file {
                        let _ = writeln!(f, "{formatted}");
                    }
                    self.current_check_buf.push(formatted.clone());
                    self.output_lines.push(formatted);
                    self.output_scroll = self.output_lines.len().saturating_sub(1);
                }
                RunMsg::Done { idx, success, elapsed } => {
                    let check_name = self.checks[idx].name.clone();
                    self.statuses[idx] = if success {
                        let line = format!("└─ [+] OK    {check_name}  ({elapsed:.1}s)");
                        if let Some(ref mut f) = self.log_file { let _ = writeln!(f, "{line}"); let _ = writeln!(f); }
                        self.current_check_buf.clear();
                        self.output_lines.push(line);
                        CheckStatus::Passed(elapsed)
                    } else {
                        let line = format!("└─ [x] FAIL  {check_name}  ({elapsed:.1}s)");
                        if let Some(ref mut f) = self.log_file { let _ = writeln!(f, "{line}"); let _ = writeln!(f); }
                        self.current_check_buf.push(line.clone());
                        if let Some(ref mut f) = self.failed_log_file {
                            for buf_line in &self.current_check_buf {
                                let _ = writeln!(f, "{buf_line}");
                            }
                            let _ = writeln!(f);
                        }
                        self.current_check_buf.clear();
                        self.output_lines.push(line);
                        CheckStatus::Failed(elapsed)
                    };
                    self.output_lines.push(String::new());
                    self.output_scroll = self.output_lines.len().saturating_sub(1);
                    next_idx = Some(idx + 1);
                }
            }
        }

        if let Some(next) = next_idx {
            self.run_rx = None;
            self.cancel_flag = None;
            self.advance_to(next);
        }
    }

    fn advance_to(&mut self, next: usize) {
        self.mode = if next >= self.checks.len() {
            self.log_file = None;
            self.failed_log_file = None;
            Mode::Done
        } else {
            Mode::Running { idx: next }
        };
    }

    pub fn summary_counts(&self) -> (usize, usize, usize) {
        let (mut passed, mut failed, mut skipped) = (0, 0, 0);
        for s in &self.statuses {
            match s {
                CheckStatus::Passed(_) => passed += 1,
                CheckStatus::Failed(_) => failed += 1,
                CheckStatus::Skipped => skipped += 1,
                _ => {}
            }
        }
        (passed, failed, skipped)
    }
}

pub fn build_entries(checks: &[CheckDef], groups: &[GroupDef]) -> Vec<ListEntry> {
    let mut entries = Vec::new();
    for (group_idx, _) in groups.iter().enumerate() {
        entries.push(ListEntry::GroupHeader(group_idx));
        for (i, c) in checks.iter().enumerate() {
            if c.group_idx == group_idx {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::load_checks_config;

    fn make_app() -> App {
        let config = load_checks_config();
        let groups = config.groups;
        let checks = config.checks;
        let n: usize = checks.len();
        let entries = build_entries(&checks, &groups);
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        App {
            groups,
            selected: vec![true; n],
            statuses: vec![CheckStatus::Pending; n],
            entries,
            checks,
            list_state,
            output_lines: Vec::new(),
            output_scroll: 0,
            mode: Mode::Selecting,
            staged_files: Vec::new(),
            list_area: None,
            run_rx: None,
            cancel_flag: None,
            log_file: None,
            failed_log_file: None,
            current_check_buf: Vec::new(),
            repo_root: PathBuf::from("./"),
            log_dir: None,
            mouse_capture: false,
            setup_repo: String::new(),
            setup_branch: String::new(),
            setup_focus: 0,
            setup_error: None,
            setup_log: Vec::new(),
            current_branch: "main".to_string(),
            cpu_pct: 0.0,
            mem_pct: 0.0,
            sys: System::new(),
        }
    }

    #[test]
    fn build_entries_starts_with_group_header() {
        let config = load_checks_config();
        let entries = build_entries(&config.checks, &config.groups);
        assert!(matches!(entries[0], ListEntry::GroupHeader(_)));
    }

    #[test]
    fn build_entries_has_entry_for_every_check() {
        let config = load_checks_config();
        let n = config.checks.len();
        let entries = build_entries(&config.checks, &config.groups);
        let check_count = entries.iter().filter(|e| matches!(e, ListEntry::Check(_))).count();
        assert_eq!(check_count, n);
    }

    #[test]
    fn build_entries_group_headers_in_order() {
        let config = load_checks_config();
        let entries = build_entries(&config.checks, &config.groups);
        let header_indices: Vec<usize> = entries
            .iter()
            .filter_map(|e| if let ListEntry::GroupHeader(idx) = e { Some(*idx) } else { None })
            .collect();
        let expected: Vec<usize> = (0..config.groups.len()).collect();
        assert_eq!(header_indices, expected);
    }

    #[test]
    fn select_all_marks_everything() {
        let mut app = make_app();
        app.selected.iter_mut().for_each(|s| *s = false);
        app.select_all();
        assert!(app.selected.iter().all(|&s| s));
    }

    #[test]
    fn select_none_clears_everything() {
        let mut app = make_app();
        app.select_none();
        assert!(app.selected.iter().all(|&s| !s));
    }

    #[test]
    fn summary_counts_pending_not_counted() {
        let app = make_app();
        let (passed, failed, skipped) = app.summary_counts();
        assert_eq!((passed, failed, skipped), (0, 0, 0));
    }

    #[test]
    fn summary_counts_mixed_statuses() {
        let mut app = make_app();
        let n = app.statuses.len();
        if n >= 1 { app.statuses[0] = CheckStatus::Passed(1.0); }
        if n >= 2 { app.statuses[1] = CheckStatus::Failed(2.0); }
        if n >= 3 { app.statuses[2] = CheckStatus::Skipped; }
        let (passed, failed, skipped) = app.summary_counts();
        assert_eq!(passed, if n >= 1 { 1 } else { 0 });
        assert_eq!(failed, if n >= 2 { 1 } else { 0 });
        assert_eq!(skipped, if n >= 3 { 1 } else { 0 });
    }

    #[test]
    fn move_up_does_not_go_below_zero() {
        let mut app = make_app();
        app.list_state.select(Some(0));
        app.move_up();
        assert_eq!(app.list_state.selected(), Some(0));
    }

    #[test]
    fn move_down_does_not_exceed_entries() {
        let mut app = make_app();
        let last = app.entries.len() - 1;
        app.list_state.select(Some(last));
        app.move_down();
        assert_eq!(app.list_state.selected(), Some(last));
    }

    #[test]
    fn move_up_and_down_navigate() {
        let mut app = make_app();
        let n = app.entries.len();
        if n < 2 { return; }
        let mid = n / 2;
        app.list_state.select(Some(mid));
        app.move_up();
        assert_eq!(app.list_state.selected(), Some(mid - 1));
        app.move_down();
        assert_eq!(app.list_state.selected(), Some(mid));
    }

    #[test]
    fn output_scroll_up_stops_at_zero() {
        let mut app = make_app();
        app.output_scroll = 0;
        app.output_scroll_up();
        assert_eq!(app.output_scroll, 0);
    }

    #[test]
    fn output_scroll_down_bounded_by_lines() {
        let mut app = make_app();
        app.output_lines = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        app.output_scroll = 0;
        app.output_scroll_down();
        assert!(app.output_scroll <= app.output_lines.len().saturating_sub(1));
    }

    #[test]
    fn group_state_counts_correctly() {
        let mut app = make_app();
        // group 0 = python (first group in config)
        let group0_indices: Vec<usize> = app.checks.iter().enumerate()
            .filter(|(_, c)| c.group_idx == 0)
            .map(|(i, _)| i)
            .collect();
        for i in &group0_indices {
            app.selected[*i] = false;
        }
        let (sel, tot) = app.group_state(0);
        assert_eq!(sel, 0);
        assert_eq!(tot, group0_indices.len());
    }

    #[test]
    fn toggle_group_selects_all_when_partial() {
        let mut app = make_app();
        let first = app.checks.iter().position(|c| c.group_idx == 0).unwrap();
        app.selected[first] = false;
        app.toggle_group(0);
        let (sel, tot) = app.group_state(0);
        assert_eq!(sel, tot, "all group-0 checks should be selected after toggle");
    }

    #[test]
    fn toggle_group_deselects_all_when_fully_selected() {
        let mut app = make_app();
        app.toggle_group(0);
        let (sel, _) = app.group_state(0);
        assert_eq!(sel, 0, "all group-0 checks should be deselected after toggle");
    }

    #[test]
    fn detect_current_branch_returns_question_mark_for_nondir() {
        let result = detect_current_branch(&PathBuf::from("/nonexistent/path/xyz"));
        assert_eq!(result, "?");
    }
}

pub fn checkout_branch_or_pr(
    repo: &PathBuf,
    branch_or_pr: &str,
) -> Result<Vec<String>, (Vec<String>, String)> {
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
