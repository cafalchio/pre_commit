use std::io::{self, BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
        MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};

// ---------------------------------------------------------------------------
// CLI config
// ---------------------------------------------------------------------------
struct Config {
    /// Absolute path to the mcp-context-forge repository root.
    repo: PathBuf,
    /// Optional path to the Python virtual environment.
    venv: Option<PathBuf>,
    /// If set, run only this check in headless (non-TUI) mode and exit.
    run_check: Option<String>,
}

fn parse_args() -> Result<Config, String> {
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

// ---------------------------------------------------------------------------
// Groups
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, PartialEq, Eq)]
enum Group {
    Python,
    Rust,
    Ui,
    /// Checks that don't belong to a specific stack — always shown.
    All,
}

impl Group {
    const ALL: [Group; 4] = [Group::Python, Group::Rust, Group::Ui, Group::All];

    fn label(self) -> &'static str {
        match self {
            Group::Python => "Python",
            Group::Rust => "Rust",
            Group::Ui => "UI",
            Group::All => "All",
        }
    }

    fn color(self) -> Color {
        match self {
            Group::Python => Color::Blue,
            Group::Rust => Color::Red,
            Group::Ui => Color::Magenta,
            Group::All => Color::White,
        }
    }
}

// ---------------------------------------------------------------------------
// Check definitions
// ---------------------------------------------------------------------------
struct CheckDef {
    name: &'static str,
    description: &'static str,
    cmd: Vec<&'static str>,
    only_when_staged: Option<&'static str>,
    advisory: bool,
    group: Group,
}

fn all_checks() -> Vec<CheckDef> {
    vec![
        // ── Python ──────────────────────────────────────────────────────────
        CheckDef {
            name: "ruff:mcpgateway",
            description: "Ruff linter (mcpgateway/)",
            cmd: vec!["uv", "run", "ruff", "check", "mcpgateway"],
            only_when_staged: Some("mcpgateway/"),
            advisory: false,
            group: Group::Python,
        },
        CheckDef {
            name: "ruff:plugins",
            description: "Ruff linter (plugins/)",
            cmd: vec!["uv", "run", "ruff", "check", "plugins"],
            only_when_staged: Some("plugins/"),
            advisory: false,
            group: Group::Python,
        },
        CheckDef {
            name: "black:check",
            description: "Black formatter check (mcpgateway/ plugins/)",
            cmd: vec!["uv", "run", "black", "--check", "mcpgateway", "plugins"],
            only_when_staged: Some("mcpgateway/"),
            advisory: false,
            group: Group::Python,
        },
        CheckDef {
            name: "isort:check",
            description: "Import order check (mcpgateway/ plugins/)",
            cmd: vec![
                "uv", "run", "isort", "--check", "--profile=black", "mcpgateway", "plugins",
            ],
            only_when_staged: Some("mcpgateway/"),
            advisory: false,
            group: Group::Python,
        },
        CheckDef {
            name: "bandit",
            description: "Bandit security scan (medium+, high-confidence)",
            cmd: vec![
                "uv", "run", "bandit", "-r", "mcpgateway", "-lll",
                "--confidence-level", "HIGH",
            ],
            only_when_staged: Some("mcpgateway/"),
            advisory: false,
            group: Group::Python,
        },
        CheckDef {
            name: "flake8:mcpgateway",
            description: "Flake8 PEP-8 / logic errors (mcpgateway/)",
            cmd: vec!["uv", "run", "flake8", "mcpgateway"],
            only_when_staged: Some("mcpgateway/"),
            advisory: false,
            group: Group::Python,
        },
        CheckDef {
            name: "flake8:plugins",
            description: "Flake8 PEP-8 / logic errors (plugins/)",
            cmd: vec!["uv", "run", "flake8", "plugins"],
            only_when_staged: Some("plugins/"),
            advisory: false,
            group: Group::Python,
        },
        CheckDef {
            name: "interrogate",
            description: "Docstring coverage 100% (mcpgateway/)",
            cmd: vec!["uv", "run", "interrogate", "-vv", "mcpgateway", "--fail-under", "100"],
            only_when_staged: Some("mcpgateway/"),
            advisory: false,
            group: Group::Python,
        },
        CheckDef {
            name: "pylint:mcpgateway",
            description: "Pylint static analysis (mcpgateway/)",
            cmd: vec![
                "uv", "run", "pylint", "mcpgateway",
                "--rcfile=.pylintrc.mcpgateway", "--fail-on", "E", "--fail-under=10",
            ],
            only_when_staged: Some("mcpgateway/"),
            advisory: false,
            group: Group::Python,
        },
        CheckDef {
            name: "pylint:plugins",
            description: "Pylint static analysis (plugins/)",
            cmd: vec![
                "uv", "run", "pylint", "plugins",
                "--rcfile=.pylintrc.plugins", "--fail-on", "E", "--fail-under=10",
            ],
            only_when_staged: Some("plugins/"),
            advisory: false,
            group: Group::Python,
        },
        CheckDef {
            name: "radon",
            description: "Cyclomatic / maintainability complexity (advisory)",
            cmd: vec![
                "sh", "-c",
                "uv run radon cc mcpgateway --min C --show-complexity && uv run radon mi mcpgateway --min B",
            ],
            only_when_staged: Some("mcpgateway/"),
            advisory: true,
            group: Group::Python,
        },
        CheckDef {
            name: "pytest:coverage",
            description: "pytest + 95% line/branch coverage",
            cmd: vec![
                "uv", "run", "pytest", "-n", "auto",
                "--ignore=tests/fuzz",
                "--ignore=tests/e2e/test_entra_id_integration.py",
                "--cov=mcpgateway", "--cov-branch",
                "--cov-report=term-missing", "--cov-fail-under=95", "-q",
            ],
            only_when_staged: Some("mcpgateway/"),
            advisory: false,
            group: Group::Python,
        },
        CheckDef {
            name: "pytest:doctests",
            description: "Doctests with 30% coverage floor (mcpgateway/)",
            cmd: vec![
                "uv", "run", "pytest", "-n", "auto",
                "--doctest-modules", "mcpgateway/",
                "--cov=mcpgateway", "--cov-fail-under=30", "--tb=short", "-q",
            ],
            only_when_staged: Some("mcpgateway/"),
            advisory: false,
            group: Group::Python,
        },
        // ── Rust ────────────────────────────────────────────────────────────
        CheckDef {
            name: "cargo:fmt-check",
            description: "Rust formatting check (plugins_rust/)",
            cmd: vec!["sh", "-c", "cd plugins_rust && cargo fmt -- --check"],
            only_when_staged: Some("plugins_rust/"),
            advisory: false,
            group: Group::Rust,
        },
        CheckDef {
            name: "cargo:clippy",
            description: "Rust Clippy lint (plugins_rust/)",
            cmd: vec!["sh", "-c", "cd plugins_rust && cargo clippy -- -D warnings"],
            only_when_staged: Some("plugins_rust/"),
            advisory: false,
            group: Group::Rust,
        },
        CheckDef {
            name: "cargo:test",
            description: "Rust unit tests (plugins_rust/)",
            cmd: vec!["sh", "-c", "cd plugins_rust && cargo test"],
            only_when_staged: Some("plugins_rust/"),
            advisory: false,
            group: Group::Rust,
        },
        // ── UI ──────────────────────────────────────────────────────────────
        CheckDef {
            name: "vitest",
            description: "JavaScript unit tests (Vitest)",
            cmd: vec!["npx", "vitest", "run"],
            only_when_staged: Some("mcpgateway/static/"),
            advisory: false,
            group: Group::Ui,
        },
        // ── All (stack-agnostic) ─────────────────────────────────────────────
        CheckDef {
            name: "yamllint",
            description: "YAML syntax check",
            cmd: vec!["uv", "run", "yamllint", "-c", ".yamllint", "."],
            only_when_staged: None,
            advisory: false,
            group: Group::All,
        },
    ]
}

// ---------------------------------------------------------------------------
// Flat list entries (group header + individual check rows)
// ---------------------------------------------------------------------------
#[derive(Clone)]
enum ListEntry {
    GroupHeader(Group),
    Check(usize), // index into App::checks
}

fn build_entries(checks: &[CheckDef]) -> Vec<ListEntry> {
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

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------
#[derive(Clone)]
enum CheckStatus {
    Pending,
    Running,
    Passed(f64),
    Failed(f64),
    Skipped,
    Advisory(f64),
}

enum Mode {
    /// Initial path-entry screen.
    Setup,
    Selecting,
    /// `idx` = index into `checks` currently being processed.
    Running { idx: usize },
    Done,
}

enum RunMsg {
    Line(String),
    Done { success: bool, elapsed: f64 },
}

struct App {
    checks: Vec<CheckDef>,
    entries: Vec<ListEntry>,
    selected: Vec<bool>,
    statuses: Vec<CheckStatus>,
    list_state: ListState,
    output_lines: Vec<String>,
    output_scroll: usize,
    mode: Mode,
    staged_files: Vec<String>,
    list_area: Option<Rect>,
    run_rx: Option<mpsc::Receiver<RunMsg>>,
    repo_root: PathBuf,
    venv: Option<PathBuf>,
    // ── Setup screen state ───────────────────────────────────────────────────
    setup_repo: String,
    setup_venv: String,
    setup_focus: usize, // 0 = repo field, 1 = venv field
    setup_error: Option<String>,
}

impl App {
    fn new(config: Config) -> Self {
        let checks = all_checks();
        let n = checks.len();
        let entries = build_entries(&checks);
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        let setup_repo = config.repo.to_string_lossy().into_owned();
        let setup_venv = config
            .venv
            .as_ref()
            .map(|v| v.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
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
            repo_root: config.repo,
            venv: config.venv,
            setup_repo,
            setup_venv,
            setup_focus: 0,
            setup_error: None,
        }
    }

    /// Validate the setup fields and transition to Selecting.
    fn confirm_setup(&mut self) {
        let repo = PathBuf::from(self.setup_repo.trim());
        if !repo.is_dir() {
            self.setup_error = Some(format!(
                "path does not exist: {}",
                repo.display()
            ));
            return;
        }
        let venv = if self.setup_venv.trim().is_empty() {
            None
        } else {
            let v = PathBuf::from(self.setup_venv.trim());
            if !v.is_dir() {
                self.setup_error =
                    Some(format!("venv path does not exist: {}", v.display()));
                return;
            }
            Some(v)
        };
        self.repo_root = repo;
        self.venv = venv;
        self.staged_files = get_staged_files(&self.repo_root);
        self.setup_error = None;
        self.mode = Mode::Selecting;
    }

    fn setup_type_char(&mut self, c: char) {
        match self.setup_focus {
            0 => self.setup_repo.push(c),
            _ => self.setup_venv.push(c),
        }
        self.setup_error = None;
    }

    fn setup_backspace(&mut self) {
        match self.setup_focus {
            0 => { self.setup_repo.pop(); }
            _ => { self.setup_venv.pop(); }
        }
        self.setup_error = None;
    }

    // ── Group helpers ────────────────────────────────────────────────────────

    /// (selected_count, total_count) for a group.
    fn group_state(&self, group: Group) -> (usize, usize) {
        self.checks
            .iter()
            .enumerate()
            .filter(|(_, c)| c.group == group)
            .fold((0, 0), |(sel, tot), (i, _)| {
                (sel + self.selected[i] as usize, tot + 1)
            })
    }

    /// Toggle all checks in a group: select-all if not all selected, else deselect-all.
    fn toggle_group(&mut self, group: Group) {
        let (sel, tot) = self.group_state(group);
        let new_val = sel < tot;
        for (i, c) in self.checks.iter().enumerate() {
            if c.group == group {
                self.selected[i] = new_val;
            }
        }
    }

    // ── Navigation & selection ───────────────────────────────────────────────

    fn toggle_current(&mut self) {
        if let Some(ei) = self.list_state.selected() {
            match self.entries[ei].clone() {
                ListEntry::GroupHeader(g) => self.toggle_group(g),
                ListEntry::Check(ci) => self.selected[ci] = !self.selected[ci],
            }
        }
    }

    fn select_all(&mut self) {
        self.selected.iter_mut().for_each(|s| *s = true);
    }

    fn select_none(&mut self) {
        self.selected.iter_mut().for_each(|s| *s = false);
    }

    fn move_up(&mut self) {
        let i = self.list_state.selected().unwrap_or(0);
        if i > 0 {
            self.list_state.select(Some(i - 1));
        }
    }

    fn move_down(&mut self) {
        let i = self.list_state.selected().unwrap_or(0);
        if i + 1 < self.entries.len() {
            self.list_state.select(Some(i + 1));
        }
    }

    fn handle_mouse_click(&mut self, col: u16, row: u16) {
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

    fn output_scroll_up(&mut self) {
        self.output_scroll = self.output_scroll.saturating_sub(3);
    }

    fn output_scroll_down(&mut self) {
        let max = self.output_lines.len().saturating_sub(1);
        self.output_scroll = (self.output_scroll + 3).min(max);
    }

    // ── Running ──────────────────────────────────────────────────────────────

    fn start_running(&mut self) {
        for s in &mut self.statuses {
            *s = CheckStatus::Pending;
        }
        self.output_lines.clear();
        self.output_lines.push(format!(
            "Staged files: {}",
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

    /// Called every frame while Running. Loops over consecutive skips, then polls output.
    fn tick_running(&mut self) {
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
                    continue; // was a skip, advance immediately
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

        // In the TUI the user explicitly selected checks to run — no stage filter.
        if !selected {
            self.statuses[idx] = CheckStatus::Skipped;
            self.output_lines
                .push(format!("[-] SKIP  {check_name}  (not selected)"));
            self.output_lines.push(String::new());
            self.advance(idx);
            return;
        }

        self.statuses[idx] = CheckStatus::Running;
        self.output_lines
            .push(format!("┌─ Running: {check_name} "));
        self.output_scroll = self.output_lines.len().saturating_sub(1);

        let (tx, rx) = mpsc::channel::<RunMsg>();
        self.run_rx = Some(rx);

        let repo_root = self.repo_root.clone();
        let venv = self.venv.clone();

        thread::spawn(move || {
            let (prog, args) = cmd.split_first().expect("empty cmd");
            let start = Instant::now();

            let mut command = Command::new(prog);
            command
                .args(args)
                .current_dir(&repo_root)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            // Prepend <venv>/bin to PATH so tools are found without `uv run`.
            if let Some(ref v) = venv {
                let venv_bin = v.join("bin");
                let base_path = std::env::var("PATH").unwrap_or_default();
                command
                    .env("VIRTUAL_ENV", v)
                    .env("PATH", format!("{}:{base_path}", venv_bin.display()));
            }

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

            let status = child.wait().ok();
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
                self.output_lines
                    .push(format!("└─ [+] OK    {check_name}  ({elapsed:.1}s)"));
                CheckStatus::Passed(elapsed)
            } else if check_advisory {
                self.output_lines
                    .push(format!("└─ [!] WARN  {check_name}  ({elapsed:.1}s)"));
                CheckStatus::Advisory(elapsed)
            } else {
                self.output_lines
                    .push(format!("└─ [x] FAIL  {check_name}  ({elapsed:.1}s)"));
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

    fn summary_counts(&self) -> (usize, usize, usize, usize) {
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

fn get_staged_files(repo: &PathBuf) -> Vec<String> {
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

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------
fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5), Constraint::Length(3)])
        .split(area);

    // ── Setup screen — rendered entirely separately ───────────────────────────
    if let Mode::Setup = &app.mode {
        draw_setup(f, app, area);
        return;
    }

    // ── Title ─────────────────────────────────────────────────────────────────
    let state_label = match &app.mode {
        Mode::Setup => unreachable!(),
        Mode::Selecting => "Select & Run",
        Mode::Running { .. } => "Running…",
        Mode::Done => "Done",
    };
    let venv_label = match &app.venv {
        Some(v) => format!("  venv: {}", v.display()),
        None => String::new(),
    };
    let title_text = format!(
        " {}  [{}]{venv_label} ",
        app.repo_root.display(),
        state_label,
    );
    f.render_widget(
        Paragraph::new(title_text).block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ),
        root[0],
    );

    // ── Main split ────────────────────────────────────────────────────────────
    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(root[1]);

    app.list_area = Some(main[0]);

    // ── Check list ────────────────────────────────────────────────────────────
    let running_check_idx = match &app.mode {
        Mode::Running { idx } => Some(*idx),
        _ => None,
    };

    let items: Vec<ListItem> = app.entries.iter().map(|entry| {
        match entry {
            ListEntry::GroupHeader(group) => {
                let (sel, tot) = app.group_state(*group);
                let cb = if sel == 0 { "[ ]" } else if sel == tot { "[x]" } else { "[-]" };
                let color = group.color();
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{cb} ─ {} ({sel}/{tot})", group.label()),
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ),
                ]))
            }
            ListEntry::Check(ci) => {
                let ci = *ci;
                let check = &app.checks[ci];
                let cb = if app.selected[ci] { "[x]" } else { "[ ]" };
                let cb_style = if app.selected[ci] {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                let (icon, item_style, elapsed_str) = match &app.statuses[ci] {
                    CheckStatus::Pending => {
                        let s = if running_check_idx == Some(ci) {
                            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        };
                        ("   ", s, String::new())
                    }
                    CheckStatus::Running => (
                        ">> ",
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                        String::new(),
                    ),
                    CheckStatus::Passed(t) => (
                        "✓  ",
                        Style::default().fg(Color::Green),
                        format!(" {t:.1}s"),
                    ),
                    CheckStatus::Failed(t) => (
                        "✗  ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        format!(" {t:.1}s"),
                    ),
                    CheckStatus::Skipped => ("---", Style::default().fg(Color::DarkGray), String::new()),
                    CheckStatus::Advisory(t) => (
                        "WRN",
                        Style::default().fg(Color::Yellow),
                        format!(" {t:.1}s"),
                    ),
                };

                ListItem::new(Line::from(vec![
                    Span::raw("  "), // indent under group
                    Span::styled(format!("{cb} "), cb_style),
                    Span::styled(format!("{icon} "), item_style),
                    Span::styled(check.name, item_style),
                    Span::styled(elapsed_str, Style::default().fg(Color::DarkGray)),
                ]))
            }
        }
    }).collect();

    let total_sel: usize = app.selected.iter().filter(|&&s| s).count();
    let list_title = format!(" Checks ({}/{} selected) ", total_sel, app.checks.len());
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(list_title))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(list, main[0], &mut app.list_state);

    // ── Output / description panel ────────────────────────────────────────────
    let output_content: Vec<Line> = if app.output_lines.is_empty() {
        // Describe the highlighted entry while idle
        match app.list_state.selected().map(|ei| &app.entries[ei]) {
            Some(ListEntry::GroupHeader(group)) => {
                let (sel, tot) = app.group_state(*group);
                vec![
                    Line::from(vec![
                        Span::styled("Group: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::styled(group.label(), Style::default().fg(group.color()).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(vec![
                        Span::styled("Checks selected: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(format!("{sel}/{tot}")),
                    ]),
                    Line::raw(""),
                    Line::raw("Space / click: toggle all checks in this group"),
                ]
            }
            Some(ListEntry::Check(ci)) => {
                let check = &app.checks[*ci];
                let staged_note = match check.only_when_staged {
                    Some(p) => format!("Only runs when '{p}' files are staged."),
                    None => "Runs unconditionally.".to_string(),
                };
                vec![
                    Line::from(vec![
                        Span::styled("Name:  ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(check.name),
                    ]),
                    Line::from(vec![
                        Span::styled("About: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(check.description),
                    ]),
                    Line::from(vec![
                        Span::styled("When:  ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(staged_note),
                    ]),
                    Line::raw(""),
                    Line::from(vec![
                        Span::styled("Cmd:   ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::styled(check.cmd.join(" "), Style::default().fg(Color::Yellow)),
                    ]),
                ]
            }
            None => vec![],
        }
    } else {
        app.output_lines
            .iter()
            .skip(app.output_scroll)
            .map(|l| {
                let style = if l.starts_with("└─ [+]") || l.starts_with("[+]") {
                    Style::default().fg(Color::Green)
                } else if l.starts_with("└─ [x]") || l.starts_with("[x]") {
                    Style::default().fg(Color::Red)
                } else if l.starts_with("└─ [!]") || l.starts_with("[!]") {
                    Style::default().fg(Color::Yellow)
                } else if l.starts_with("[-]") {
                    Style::default().fg(Color::DarkGray)
                } else if l.starts_with("┌─") {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                Line::from(Span::styled(l.as_str(), style))
            })
            .collect()
    };

    let output_title = if app.output_lines.is_empty() {
        " Description ".to_string()
    } else {
        format!(" Output  {}/{} ", app.output_scroll + 1, app.output_lines.len())
    };

    f.render_widget(
        Paragraph::new(output_content)
            .block(Block::default().borders(Borders::ALL).title(output_title))
            .wrap(Wrap { trim: false }),
        main[1],
    );

    // ── Status bar ────────────────────────────────────────────────────────────
    if let Mode::Done = &app.mode {
        let (passed, failed, skipped, advisory) = app.summary_counts();
        let summary = format!(
            " Done — OK: {passed}  FAIL: {failed}  WARN: {advisory}  SKIP: {skipped}  | r: run again | q: quit "
        );
        let color = if failed > 0 { Color::Red } else if advisory > 0 { Color::Yellow } else { Color::Green };
        f.render_widget(
            Paragraph::new(summary).block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(color).add_modifier(Modifier::BOLD)),
            ),
            root[2],
        );
        return;
    }

    let (staged_info, help_text) = match &app.mode {
        Mode::Setup => unreachable!(),
        Mode::Selecting => {
            let n = app.staged_files.len();
            (
                format!(" {n} staged file(s) "),
                " ↑↓/jk: move | Space/Click: toggle | a: all | n: none | Enter/r: run | q: quit ".to_string(),
            )
        }
        Mode::Running { idx } => (
            format!(" Running {}/{} ", idx + 1, app.checks.len()),
            " Running… PgUp/PgDn or scroll to view output ".to_string(),
        ),
        Mode::Done => unreachable!(),
    };

    let status_bar = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(22), Constraint::Min(10)])
        .split(root[2]);

    f.render_widget(
        Paragraph::new(staged_info).block(
            Block::default().borders(Borders::ALL).style(Style::default().fg(Color::Cyan)),
        ),
        status_bar[0],
    );
    f.render_widget(
        Paragraph::new(help_text).block(
            Block::default().borders(Borders::ALL).style(Style::default().fg(Color::DarkGray)),
        ),
        status_bar[1],
    );
}

// ---------------------------------------------------------------------------
// Setup screen
// ---------------------------------------------------------------------------
fn draw_setup(f: &mut Frame, app: &App, area: Rect) {
    // Center a fixed-width form.
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Length(16),
            Constraint::Min(0),
        ])
        .split(area);
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(10),
            Constraint::Min(40),
            Constraint::Percentage(10),
        ])
        .split(vert[1]);
    let form_area = horiz[1];

    // Outer border.
    f.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title(" mcp-context-forge Pre-commit — Project Setup ")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        form_area,
    );

    // Inner layout: label + repo field + gap + label + venv field + error + hint.
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // "Project path:" label
            Constraint::Length(3), // repo input box
            Constraint::Length(1), // blank
            Constraint::Length(1), // "Python venv:" label
            Constraint::Length(3), // venv input box
            Constraint::Length(1), // error
            Constraint::Length(1), // hint
        ])
        .margin(1)
        .split(form_area);

    // Repo label.
    f.render_widget(
        Paragraph::new("Project path (required):").style(Style::default().add_modifier(Modifier::BOLD)),
        inner[0],
    );

    // Repo input.
    let repo_content = format!("{}\u{2588}", app.setup_repo); // append block cursor
    let repo_style = if app.setup_focus == 0 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    f.render_widget(
        Paragraph::new(if app.setup_focus == 0 { repo_content } else { app.setup_repo.clone() })
            .block(Block::default().borders(Borders::ALL).style(repo_style)),
        inner[1],
    );

    // Venv label.
    f.render_widget(
        Paragraph::new("Python venv path (optional):").style(Style::default().add_modifier(Modifier::BOLD)),
        inner[3],
    );

    // Venv input.
    let venv_content = format!("{}\u{2588}", app.setup_venv);
    let venv_style = if app.setup_focus == 1 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    f.render_widget(
        Paragraph::new(if app.setup_focus == 1 { venv_content } else { app.setup_venv.clone() })
            .block(Block::default().borders(Borders::ALL).style(venv_style)),
        inner[4],
    );

    // Error line.
    if let Some(err) = &app.setup_error {
        f.render_widget(
            Paragraph::new(format!("  ✗ {err}"))
                .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            inner[5],
        );
    }

    // Hint.
    f.render_widget(
        Paragraph::new("  Tab/↑↓: switch field   Enter: confirm   Esc/q: quit")
            .style(Style::default().fg(Color::DarkGray)),
        inner[6],
    );
}

// ---------------------------------------------------------------------------
// Headless single-check runner (--check flag)
// ---------------------------------------------------------------------------
fn run_headless(check_name: &str, repo: &PathBuf, venv: &Option<PathBuf>) {
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

    let mut command = Command::new(prog);
    command
        .args(args)
        .current_dir(repo)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(v) = venv {
        let venv_bin = v.join("bin");
        let base = std::env::var("PATH").unwrap_or_default();
        command
            .env("VIRTUAL_ENV", v)
            .env("PATH", format!("{}:{base}", venv_bin.display()));
    }

    let mut child = command.spawn().unwrap_or_else(|e| {
        eprintln!("spawn error: {e}");
        std::process::exit(2);
    });

    // Stream stdout and stderr to our terminal in real-time via reader threads.
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

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------
fn main() -> io::Result<()> {
    let config = parse_args().unwrap_or_else(|e| {
        eprintln!("error: {e}");
        eprintln!("Run with --help for usage.");
        std::process::exit(1);
    });

    // Headless mode: run one check and exit, no TUI.
    if let Some(ref name) = config.run_check {
        run_headless(name, &config.repo, &config.venv);
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config);

    loop {
        terminal.draw(|f| draw(f, &mut app))?;

        if let Mode::Running { .. } = &app.mode {
            app.tick_running();

            if event::poll(std::time::Duration::from_millis(16))? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::PageUp => app.output_scroll_up(),
                        KeyCode::PageDown => app.output_scroll_down(),
                        _ => {}
                    },
                    Event::Mouse(mouse) => match mouse.kind {
                        MouseEventKind::ScrollUp => app.output_scroll_up(),
                        MouseEventKind::ScrollDown => app.output_scroll_down(),
                        _ => {}
                    },
                    _ => {}
                }
            }
            continue;
        }

        if event::poll(std::time::Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Esc => break,
                    KeyCode::Char('q') if !matches!(&app.mode, Mode::Setup) => break,

                    // ── Setup mode ────────────────────────────────────────────
                    KeyCode::Tab | KeyCode::Down
                        if matches!(&app.mode, Mode::Setup) =>
                    {
                        app.setup_focus = 1 - app.setup_focus;
                    }
                    KeyCode::Up if matches!(&app.mode, Mode::Setup) => {
                        app.setup_focus = 1 - app.setup_focus;
                    }
                    KeyCode::Enter if matches!(&app.mode, Mode::Setup) => {
                        app.confirm_setup();
                    }
                    KeyCode::Char(c) if matches!(&app.mode, Mode::Setup) => {
                        app.setup_type_char(c);
                    }
                    KeyCode::Backspace if matches!(&app.mode, Mode::Setup) => {
                        app.setup_backspace();
                    }

                    // ── Selecting / Done modes ─────────────────────────────────
                    KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                    KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                    KeyCode::Char(' ') => {
                        if matches!(&app.mode, Mode::Selecting) {
                            app.toggle_current();
                        }
                    }
                    KeyCode::Char('a') => {
                        if matches!(&app.mode, Mode::Selecting) {
                            app.select_all();
                        }
                    }
                    KeyCode::Char('n') => {
                        if matches!(&app.mode, Mode::Selecting) {
                            app.select_none();
                        }
                    }
                    KeyCode::Enter | KeyCode::Char('r') => {
                        if matches!(&app.mode, Mode::Selecting | Mode::Done) {
                            app.start_running();
                        }
                    }
                    KeyCode::PageUp => app.output_scroll_up(),
                    KeyCode::PageDown => app.output_scroll_down(),
                    _ => {}
                },
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        if matches!(&app.mode, Mode::Selecting) {
                            app.handle_mouse_click(mouse.column, mouse.row);
                        }
                    }
                    MouseEventKind::ScrollUp => app.output_scroll_up(),
                    MouseEventKind::ScrollDown => app.output_scroll_down(),
                    _ => {}
                },
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
