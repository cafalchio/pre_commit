use super::{CheckDef, Group};

pub fn integration_checks() -> Vec<CheckDef> {
    vec![
        CheckDef {
            name: "alembic:upgrade",
            description: "Database migration upgrade validation (docker compose db, cleans up after)",
            cmd: vec![
                "sh", "-c",
                "echo '==> Starting DB via docker compose...' && \
                 docker compose up -d db && \
                 echo '==> Running upgrade validation...' && \
                 bash scripts/ci/run_upgrade_validation.sh; \
                 _EC=$?; \
                 echo '==> Cleaning up docker compose...'; \
                 docker compose down -v --remove-orphans; \
                 exit $_EC",
            ],
            only_when_staged: None,
            advisory: false,
            group: Group::Integration,
        },
        CheckDef {
            name: "playwright:smoke",
            description: "Playwright UI smoke tests (starts gateway via docker compose, cleans up after)",
            cmd: vec![
                "sh", "-c",
                "echo '==> Starting gateway via docker compose...' && \
                 docker compose up -d && \
                 echo '==> Waiting for gateway on localhost:4444...' && \
                 timeout 90 sh -c 'until curl -sf http://localhost:4444/health >/dev/null 2>&1; do sleep 2; done' && \
                 echo '==> Gateway ready, running Playwright smoke tests...' && \
                 make test-ui-ci-smoke; \
                 _EC=$?; \
                 echo '==> Cleaning up docker compose...'; \
                 docker compose down -v --remove-orphans; \
                 exit $_EC",
            ],
            only_when_staged: None,
            advisory: true,
            group: Group::Integration,
        },
        CheckDef {
            name: "maturin:build",
            description: "Build Rust extension wheels (plugins_rust/)",
            cmd: vec![
                "sh", "-c",
                "_VENV=$(mktemp -d) && \
                 uv venv \"$_VENV\" --python python3 && \
                 (cd plugins_rust && PYO3_PYTHON=\"$_VENV/bin/python\" maturin build --release --out dist); \
                 _EC=$?; rm -rf \"$_VENV\"; exit $_EC",
            ],
            only_when_staged: Some("plugins_rust/"),
            advisory: false,
            group: Group::Integration,
        },
        CheckDef {
            name: "rust:test-python",
            description: "Python integration tests for Rust extension (plugins_rust/)",
            cmd: vec![
                "sh", "-c",
                "cd plugins_rust && make dev && uv run make test-python",
            ],
            only_when_staged: Some("plugins_rust/"),
            advisory: false,
            group: Group::Integration,
        },
        CheckDef {
            name: "rust:test-differential",
            description: "Differential tests for Rust extension (plugins_rust/)",
            cmd: vec![
                "sh", "-c",
                "cd plugins_rust && uv run make test-differential",
            ],
            only_when_staged: Some("plugins_rust/"),
            advisory: false,
            group: Group::Integration,
        },
        CheckDef {
            name: "tools-rust:check",
            description: "Cargo check (tools_rust/wrapper/)",
            cmd: vec!["sh", "-c", "cd tools_rust/wrapper && make check"],
            only_when_staged: Some("tools_rust/"),
            advisory: false,
            group: Group::Integration,
        },
        CheckDef {
            name: "tools-rust:build",
            description: "Release build (tools_rust/wrapper/)",
            cmd: vec!["sh", "-c", "cd tools_rust/wrapper && make build-release"],
            only_when_staged: Some("tools_rust/"),
            advisory: false,
            group: Group::Integration,
        },
        CheckDef {
            name: "tools-rust:licenses",
            description: "Cargo-deny license check (tools_rust/wrapper/, advisory)",
            cmd: vec!["sh", "-c", "cd tools_rust/wrapper && cargo install cargo-deny --quiet && make licenses"],
            only_when_staged: Some("tools_rust/"),
            advisory: true,
            group: Group::Integration,
        },
        CheckDef {
            name: "rust:doc",
            description: "Rust documentation build (plugins_rust/, advisory)",
            cmd: vec!["sh", "-c", "cd plugins_rust && make doc"],
            only_when_staged: Some("plugins_rust/"),
            advisory: true,
            group: Group::Integration,
        },
    ]
}
