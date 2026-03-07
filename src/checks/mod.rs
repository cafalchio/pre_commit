pub mod all;
pub mod python;
pub mod rust;
pub mod ui;

use ratatui::style::Color;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Python,
    Rust,
    Ui,
    /// Checks that don't belong to a specific stack — always shown.
    All,
}

impl Group {
    pub const ALL: [Group; 4] = [Group::Python, Group::Rust, Group::Ui, Group::All];

    pub fn label(self) -> &'static str {
        match self {
            Group::Python => "Python",
            Group::Rust => "Rust",
            Group::Ui => "UI",
            Group::All => "All",
        }
    }

    pub fn color(self) -> Color {
        match self {
            Group::Python => Color::Blue,
            Group::Rust => Color::Red,
            Group::Ui => Color::Magenta,
            Group::All => Color::White,
        }
    }
}

pub struct CheckDef {
    pub name: &'static str,
    pub description: &'static str,
    pub cmd: Vec<&'static str>,
    pub only_when_staged: Option<&'static str>,
    pub advisory: bool,
    pub group: Group,
}

/// Returns all checks in group order: Python → Rust → UI → All.
pub fn all_checks() -> Vec<CheckDef> {
    let mut checks = Vec::new();
    checks.extend(python::python_checks());
    checks.extend(rust::rust_checks());
    checks.extend(ui::ui_checks());
    checks.extend(all::general_checks());
    checks
}
