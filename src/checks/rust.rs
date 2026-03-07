use super::{CheckDef, Group};

pub fn rust_checks() -> Vec<CheckDef> {
    vec![
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
        CheckDef {
            name: "cargo:audit",
            description: "Rust security audit (plugins_rust/, advisory)",
            cmd: vec!["sh", "-c", "cd plugins_rust && cargo audit"],
            only_when_staged: Some("plugins_rust/"),
            advisory: true,
            group: Group::Rust,
        },
        // ── tools_rust/wrapper ───────────────────────────────────────────────
        CheckDef {
            name: "tools-rust:fmt-check",
            description: "Rust formatting check (tools_rust/wrapper/)",
            cmd: vec!["sh", "-c", "cd tools_rust/wrapper && cargo fmt --check"],
            only_when_staged: Some("tools_rust/"),
            advisory: false,
            group: Group::Rust,
        },
        CheckDef {
            name: "tools-rust:clippy",
            description: "Rust Clippy pedantic lint (tools_rust/wrapper/)",
            cmd: vec!["sh", "-c", "cd tools_rust/wrapper && cargo clippy -- -D warnings"],
            only_when_staged: Some("tools_rust/"),
            advisory: false,
            group: Group::Rust,
        },
        CheckDef {
            name: "tools-rust:test",
            description: "Rust unit tests (tools_rust/wrapper/)",
            cmd: vec!["sh", "-c", "cd tools_rust/wrapper && cargo test"],
            only_when_staged: Some("tools_rust/"),
            advisory: false,
            group: Group::Rust,
        },
    ]
}
