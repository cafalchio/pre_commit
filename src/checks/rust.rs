use super::{CheckDef, Group};

/// Shell snippet that finds all Rust workspace roots (Cargo.toml containing [workspace])
/// plus any standalone crates (Cargo.toml not inside another workspace).
/// Excludes target/ and .venv/ dirs. Prints one directory per line.
const FIND_RUST_PROJECTS: &str = r#"
find . -name 'Cargo.toml' \
  ! -path '*/target/*' \
  ! -path '*/.venv/*' \
  | while IFS= read -r f; do
      dir=$(dirname "$f")
      # workspace root → always include
      if grep -q '^\[workspace\]' "$f" 2>/dev/null; then
        echo "$dir"
      # standalone crate → include only if no ancestor also has a Cargo.toml
      elif ! find "$(dirname "$dir")" -maxdepth 1 -name 'Cargo.toml' 2>/dev/null | grep -q .; then
        echo "$dir"
      fi
    done | sort -u
"#;

pub fn rust_checks() -> Vec<CheckDef> {
    vec![
        CheckDef {
            name: "cargo:fmt-check",
            description: "Rust formatting check (all Rust projects)",
            cmd: vec![
                "sh", "-c",
                r#"
_FAIL=0
while IFS= read -r dir; do
  echo "==> fmt: $dir"
  (cd "$dir" && cargo fmt -- --check) || _FAIL=1
done < <(find . -name 'Cargo.toml' ! -path '*/target/*' ! -path '*/.venv/*' \
  | while IFS= read -r f; do
      dir=$(dirname "$f")
      grep -q '^\[workspace\]' "$f" 2>/dev/null && echo "$dir" && continue
      find "$(dirname "$dir")" -maxdepth 1 -name 'Cargo.toml' 2>/dev/null | grep -q . || echo "$dir"
    done | sort -u)
exit $_FAIL
"#,
            ],
            only_when_staged: None,
            advisory: false,
            group: Group::Rust,
        },
        CheckDef {
            name: "cargo:clippy",
            description: "Rust Clippy lint (all Rust projects)",
            cmd: vec![
                "sh", "-c",
                r#"
_VENV=$(mktemp -d)
uv venv "$_VENV" --python python3
_FAIL=0
while IFS= read -r dir; do
  echo "==> clippy: $dir"
  (cd "$dir" && PYO3_PYTHON="$_VENV/bin/python" cargo clippy -- -D warnings) || _FAIL=1
done < <(find . -name 'Cargo.toml' ! -path '*/target/*' ! -path '*/.venv/*' \
  | while IFS= read -r f; do
      dir=$(dirname "$f")
      grep -q '^\[workspace\]' "$f" 2>/dev/null && echo "$dir" && continue
      find "$(dirname "$dir")" -maxdepth 1 -name 'Cargo.toml' 2>/dev/null | grep -q . || echo "$dir"
    done | sort -u)
rm -rf "$_VENV"
exit $_FAIL
"#,
            ],
            only_when_staged: None,
            advisory: false,
            group: Group::Rust,
        },
        CheckDef {
            name: "cargo:test",
            description: "Rust unit tests (all Rust projects)",
            cmd: vec![
                "sh", "-c",
                r#"
_VENV=$(mktemp -d)
uv venv "$_VENV" --python python3
_FAIL=0
while IFS= read -r dir; do
  echo "==> test: $dir"
  (cd "$dir" && PYO3_PYTHON="$_VENV/bin/python" cargo test) || _FAIL=1
done < <(find . -name 'Cargo.toml' ! -path '*/target/*' ! -path '*/.venv/*' \
  | while IFS= read -r f; do
      dir=$(dirname "$f")
      grep -q '^\[workspace\]' "$f" 2>/dev/null && echo "$dir" && continue
      find "$(dirname "$dir")" -maxdepth 1 -name 'Cargo.toml' 2>/dev/null | grep -q . || echo "$dir"
    done | sort -u)
rm -rf "$_VENV"
exit $_FAIL
"#,
            ],
            only_when_staged: None,
            advisory: false,
            group: Group::Rust,
        },
        CheckDef {
            name: "cargo:audit",
            description: "Rust security audit (all Rust projects, advisory)",
            cmd: vec![
                "sh", "-c",
                r#"
_FAIL=0
while IFS= read -r dir; do
  echo "==> audit: $dir"
  (cd "$dir" && cargo audit) || _FAIL=1
done < <(find . -name 'Cargo.toml' ! -path '*/target/*' ! -path '*/.venv/*' \
  | while IFS= read -r f; do
      dir=$(dirname "$f")
      grep -q '^\[workspace\]' "$f" 2>/dev/null && echo "$dir" && continue
      find "$(dirname "$dir")" -maxdepth 1 -name 'Cargo.toml' 2>/dev/null | grep -q . || echo "$dir"
    done | sort -u)
exit $_FAIL
"#,
            ],
            only_when_staged: None,
            advisory: true,
            group: Group::Rust,
        },
    ]
}
