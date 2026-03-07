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
            description: "HTML linting (mcpgateway/templates/)",
            cmd: vec!["npx", "htmlhint", "mcpgateway/templates/*.html"],
            only_when_staged: Some("mcpgateway/templates/"),
            advisory: false,
            group: Group::Ui,
        },
        CheckDef {
            name: "stylelint",
            description: "CSS linting (mcpgateway/static/*.css)",
            cmd: vec!["npx", "stylelint", "mcpgateway/static/*.css"],
            only_when_staged: Some("mcpgateway/static/"),
            advisory: false,
            group: Group::Ui,
        },
        CheckDef {
            name: "eslint",
            description: "ESLint JavaScript linting (mcpgateway/static/*.js)",
            cmd: vec!["npx", "eslint", "mcpgateway/static/*.js"],
            only_when_staged: Some("mcpgateway/static/"),
            advisory: false,
            group: Group::Ui,
        },
        CheckDef {
            name: "jshint",
            description: "JSHint JavaScript linting (mcpgateway/static/*.js)",
            cmd: vec!["npx", "jshint", "--config", ".jshintrc", "mcpgateway/static/*.js"],
            only_when_staged: Some("mcpgateway/static/"),
            advisory: false,
            group: Group::Ui,
        },
        CheckDef {
            name: "retire",
            description: "Retire.js security scan (mcpgateway/static/, advisory)",
            cmd: vec!["npx", "retire", "--path", "mcpgateway/static"],
            only_when_staged: Some("mcpgateway/static/"),
            advisory: true,
            group: Group::Ui,
        },
        CheckDef {
            name: "jscpd",
            description: "Copy-paste detection (mcpgateway/static/ + templates/, advisory)",
            cmd: vec!["npx", "jscpd", "mcpgateway/static/", "mcpgateway/templates/"],
            only_when_staged: Some("mcpgateway/static/"),
            advisory: true,
            group: Group::Ui,
        },
        CheckDef {
            name: "nodejsscan",
            description: "NodeJSScan JS security scanner (mcpgateway/static/, advisory)",
            cmd: vec!["nodejsscan", "--directory", "./mcpgateway/static"],
            only_when_staged: Some("mcpgateway/static/"),
            advisory: true,
            group: Group::Ui,
        },
        CheckDef {
            name: "npm-audit",
            description: "npm dependency security audit (advisory)",
            cmd: vec!["npm", "audit", "--audit-level=high"],
            only_when_staged: Some("mcpgateway/static/"),
            advisory: true,
            group: Group::Ui,
        },
    ]
}
