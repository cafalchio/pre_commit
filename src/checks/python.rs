use super::{CheckDef, Group};

pub fn python_checks() -> Vec<CheckDef> {
    vec![
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
            description: "Black formatter check on changed .py files (mcpgateway/ plugins/)",
            cmd: vec![
                "sh", "-c",
                "FILES=$(git diff --name-only HEAD 2>/dev/null | grep -E '^(mcpgateway|plugins)/.*\\.py$' | tr '\\n' ' '); \
                 [ -z \"$FILES\" ] && echo 'No changed Python files — skipping black' && exit 0; \
                 echo \"Running black on: $FILES\"; \
                 uv run black --check $FILES",
            ],
            only_when_staged: Some("mcpgateway/"),
            advisory: false,
            group: Group::Python,
        },
        CheckDef {
            name: "isort:check",
            description: "Import order check on changed .py files (mcpgateway/ plugins/)",
            cmd: vec![
                "sh", "-c",
                "FILES=$(git diff --name-only HEAD 2>/dev/null | grep -E '^(mcpgateway|plugins)/.*\\.py$' | tr '\\n' ' '); \
                 [ -z \"$FILES\" ] && echo 'No changed Python files — skipping isort' && exit 0; \
                 echo \"Running isort on: $FILES\"; \
                 uv run isort --check --profile=black $FILES",
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
                "--confidence-level", "high",
            ],
            only_when_staged: Some("mcpgateway/"),
            advisory: false,
            group: Group::Python,
        },
        CheckDef {
            name: "flake8:mcpgateway",
            description: "Flake8 PEP-8 / logic errors on changed .py files (mcpgateway/)",
            cmd: vec![
                "sh", "-c",
                "FILES=$(git diff --name-only HEAD 2>/dev/null | grep -E '^mcpgateway/.*\\.py$' | tr '\\n' ' '); \
                 [ -z \"$FILES\" ] && echo 'No changed Python files — skipping flake8' && exit 0; \
                 echo \"Running flake8 on: $FILES\"; \
                 uv run flake8 $FILES -v",
            ],
            only_when_staged: Some("mcpgateway/"),
            advisory: false,
            group: Group::Python,
        },
        CheckDef {
            name: "flake8:plugins",
            description: "Flake8 PEP-8 / logic errors on changed .py files (plugins/)",
            cmd: vec![
                "sh", "-c",
                "FILES=$(git diff --name-only HEAD 2>/dev/null | grep -E '^plugins/.*\\.py$' | tr '\\n' ' '); \
                 [ -z \"$FILES\" ] && echo 'No changed Python files — skipping flake8' && exit 0; \
                 echo \"Running flake8 on: $FILES\"; \
                 uv run flake8 $FILES -v",
            ],
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
            description: "Pylint on changed .py files (mcpgateway/)",
            cmd: vec![
                "sh", "-c",
                "FILES=$(git diff --name-only HEAD 2>/dev/null | grep -E '^mcpgateway/.*\\.py$' | tr '\\n' ' '); \
                 [ -z \"$FILES\" ] && echo 'No changed mcpgateway Python files — skipping pylint' && exit 0; \
                 echo \"Running pylint on: $FILES\"; \
                 uv run pylint $FILES --rcfile=.pylintrc.mcpgateway --fail-on E --fail-under=10",
            ],
            only_when_staged: Some("mcpgateway/"),
            advisory: false,
            group: Group::Python,
        },
        CheckDef {
            name: "pylint:plugins",
            description: "Pylint on changed .py files (plugins/)",
            cmd: vec![
                "sh", "-c",
                "FILES=$(git diff --name-only HEAD 2>/dev/null | grep -E '^plugins/.*\\.py$' | tr '\\n' ' '); \
                 [ -z \"$FILES\" ] && echo 'No changed plugins Python files — skipping pylint' && exit 0; \
                 echo \"Running pylint on: $FILES\"; \
                 uv run pylint $FILES --rcfile=.pylintrc.plugins --fail-on E --fail-under=10",
            ],
            only_when_staged: Some("plugins/"),
            advisory: false,
            group: Group::Python,
        },
        CheckDef {
            name: "interrogate:plugins",
            description: "Docstring coverage 100% (plugins/)",
            cmd: vec!["uv", "run", "interrogate", "-vv", "plugins", "--fail-under", "100"],
            only_when_staged: Some("plugins/"),
            advisory: false,
            group: Group::Python,
        },
        CheckDef {
            name: "unimport:mcpgateway",
            description: "Unused import detection (mcpgateway/)",
            cmd: vec!["uv", "run", "unimport", "mcpgateway"],
            only_when_staged: Some("mcpgateway/"),
            advisory: false,
            group: Group::Python,
        },
        CheckDef {
            name: "unimport:plugins",
            description: "Unused import detection (plugins/)",
            cmd: vec!["uv", "run", "unimport", "plugins"],
            only_when_staged: Some("plugins/"),
            advisory: false,
            group: Group::Python,
        },
        CheckDef {
            name: "vulture:mcpgateway",
            description: "Dead code detection (mcpgateway/, advisory)",
            cmd: vec![
                "uv", "run", "vulture", "mcpgateway",
                "--min-confidence", "80",
                "--exclude", "*_pb2.py,*_pb2_grpc.py",
            ],
            only_when_staged: Some("mcpgateway/"),
            advisory: true,
            group: Group::Python,
        },
        CheckDef {
            name: "vulture:plugins",
            description: "Dead code detection (plugins/, advisory)",
            cmd: vec![
                "uv", "run", "vulture", "plugins",
                "--min-confidence", "80",
                "--exclude", "*_pb2.py,*_pb2_grpc.py",
            ],
            only_when_staged: Some("plugins/"),
            advisory: true,
            group: Group::Python,
        },
        CheckDef {
            name: "radon:mcpgateway",
            description: "Cyclomatic / maintainability complexity mcpgateway/ (advisory)",
            cmd: vec![
                "sh", "-c",
                "uv run radon cc mcpgateway --min C --show-complexity && uv run radon mi mcpgateway --min B",
            ],
            only_when_staged: Some("mcpgateway/"),
            advisory: true,
            group: Group::Python,
        },
        CheckDef {
            name: "radon:plugins",
            description: "Cyclomatic / maintainability complexity plugins/ (advisory)",
            cmd: vec![
                "sh", "-c",
                "uv run radon cc plugins --min C --show-complexity && uv run radon mi plugins --min B",
            ],
            only_when_staged: Some("plugins/"),
            advisory: true,
            group: Group::Python,
        },
        CheckDef {
            name: "diff-cover",
            description: "95% coverage on changed lines vs origin/main (advisory)",
            cmd: vec![
                "sh", "-c",
                "N=$(( $(nproc) - 4 )); [ $N -lt 1 ] && N=1; \
                 uv run pytest -n $N --ignore=tests/fuzz --ignore=tests/e2e/test_entra_id_integration.py --cov=mcpgateway --cov-branch --cov-report=xml -q && \
                 uv run diff-cover coverage.xml --compare-branch=origin/main --fail-under=95",
            ],
            only_when_staged: Some("mcpgateway/"),
            advisory: true,
            group: Group::Python,
        },
        CheckDef {
            name: "pytest:coverage",
            description: "pytest + 95% line/branch coverage (nproc-4 workers)",
            cmd: vec![
                "sh", "-c",
                "N=$(( $(nproc) - 4 )); [ $N -lt 1 ] && N=1; \
                 uv run pytest -n $N \
                   --ignore=tests/fuzz \
                   --ignore=tests/e2e/test_entra_id_integration.py \
                   --cov=mcpgateway --cov-branch \
                   --cov-report=term-missing --cov-fail-under=95 -q",
            ],
            only_when_staged: Some("mcpgateway/"),
            advisory: false,
            group: Group::Python,
        },
        CheckDef {
            name: "pytest:doctests",
            description: "Doctests with 30% coverage floor (nproc-4 workers)",
            cmd: vec![
                "sh", "-c",
                "N=$(( $(nproc) - 4 )); [ $N -lt 1 ] && N=1; \
                 uv run pytest -n $N \
                   --doctest-modules mcpgateway/ \
                   --cov=mcpgateway --cov-fail-under=30 --tb=short -q",
            ],
            only_when_staged: Some("mcpgateway/"),
            advisory: false,
            group: Group::Python,
        },
    ]
}
