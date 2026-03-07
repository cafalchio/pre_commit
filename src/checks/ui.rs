use super::{CheckDef, Group};

pub fn ui_checks() -> Vec<CheckDef> {
    vec![
        CheckDef {
            name: "vitest",
            description: "JavaScript unit tests (npm ci + Vitest)",
            cmd: vec!["sh", "-c", "npm ci && npx vitest run"],
            only_when_staged: Some("mcpgateway/static/"),
            advisory: false,
            group: Group::Ui,
        },
        CheckDef {
            name: "htmlhint",
            description: "HTML linting on changed .html files (mcpgateway/templates/)",
            cmd: vec![
                "sh", "-c",
                "[ -d node_modules ] || npm ci; \
                 FILES=$(git diff --name-only HEAD 2>/dev/null | grep -E '^mcpgateway/templates/.*\\.html$' | tr '\\n' ' '); \
                 [ -z \"$FILES\" ] && echo 'No changed HTML files — skipping htmlhint' && exit 0; \
                 echo \"Running htmlhint on: $FILES\"; \
                 npx htmlhint $FILES",
            ],
            only_when_staged: Some("mcpgateway/templates/"),
            advisory: false,
            group: Group::Ui,
        },
        CheckDef {
            name: "stylelint",
            description: "CSS linting on changed .css files (mcpgateway/static/)",
            cmd: vec![
                "sh", "-c",
                "[ -d node_modules ] || npm ci; \
                 FILES=$(git diff --name-only HEAD 2>/dev/null | grep -E '^mcpgateway/static/.*\\.css$' | tr '\\n' ' '); \
                 [ -z \"$FILES\" ] && echo 'No changed CSS files — skipping stylelint' && exit 0; \
                 echo \"Running stylelint on: $FILES\"; \
                 npx stylelint $FILES",
            ],
            only_when_staged: Some("mcpgateway/static/"),
            advisory: false,
            group: Group::Ui,
        },
        CheckDef {
            name: "eslint",
            description: "ESLint on changed .js files (mcpgateway/static/)",
            cmd: vec![
                "sh", "-c",
                "[ -d node_modules ] || npm ci; \
                 FILES=$(git diff --name-only HEAD 2>/dev/null | grep -E '^mcpgateway/static/.*\\.js$' | tr '\\n' ' '); \
                 [ -z \"$FILES\" ] && echo 'No changed JS files — skipping eslint' && exit 0; \
                 echo \"Running eslint on: $FILES\"; \
                 npx eslint $FILES",
            ],
            only_when_staged: Some("mcpgateway/static/"),
            advisory: false,
            group: Group::Ui,
        },
        CheckDef {
            name: "jshint",
            description: "JSHint on changed .js files (mcpgateway/static/)",
            cmd: vec![
                "sh", "-c",
                "[ -d node_modules ] || npm ci; \
                 FILES=$(git diff --name-only HEAD 2>/dev/null | grep -E '^mcpgateway/static/.*\\.js$' | tr '\\n' ' '); \
                 [ -z \"$FILES\" ] && echo 'No changed JS files — skipping jshint' && exit 0; \
                 echo \"Running jshint on: $FILES\"; \
                 npx jshint --config .jshintrc $FILES",
            ],
            only_when_staged: Some("mcpgateway/static/"),
            advisory: false,
            group: Group::Ui,
        },
        CheckDef {
            name: "retire",
            description: "Retire.js security scan (mcpgateway/static/, advisory)",
            cmd: vec!["sh", "-c", "[ -d node_modules ] || npm ci; npx --yes retire --path mcpgateway/static"],
            only_when_staged: Some("mcpgateway/static/"),
            advisory: true,
            group: Group::Ui,
        },
        CheckDef {
            name: "jscpd",
            description: "Copy-paste detection (mcpgateway/static/ + templates/, advisory)",
            cmd: vec!["sh", "-c", "[ -d node_modules ] || npm ci; npx --yes jscpd mcpgateway/static/ mcpgateway/templates/"],
            only_when_staged: Some("mcpgateway/static/"),
            advisory: true,
            group: Group::Ui,
        },
        CheckDef {
            name: "nodejsscan",
            description: "NodeJSScan JS security scanner (mcpgateway/static/, advisory)",
            cmd: vec!["sh", "-c", "uv run nodejsscan --directory ./mcpgateway/static"],
            only_when_staged: Some("mcpgateway/static/"),
            advisory: true,
            group: Group::Ui,
        },
        CheckDef {
            name: "npm-audit",
            description: "npm dependency security audit (advisory)",
            cmd: vec!["sh", "-c", "[ -d node_modules ] || npm ci; npm audit --audit-level=high"],
            only_when_staged: Some("mcpgateway/static/"),
            advisory: true,
            group: Group::Ui,
        },
    ]
}
