use super::{CheckDef, Group};

pub fn general_checks() -> Vec<CheckDef> {
    vec![
        CheckDef {
            name: "yamllint",
            description: "YAML syntax check",
            cmd: vec!["uv", "run", "yamllint", "-c", ".yamllint", "."],
            only_when_staged: None,
            advisory: false,
            group: Group::All,
        },
        CheckDef {
            name: "jsonlint",
            description: "JSON syntax check (all *.json files)",
            cmd: vec![
                "sh", "-c",
                "find . -type f -name '*.json' -not -path './node_modules/*' -print0 | xargs -0 -I{} jq empty \"{}\"",
            ],
            only_when_staged: None,
            advisory: false,
            group: Group::All,
        },
        CheckDef {
            name: "tomllint",
            description: "TOML syntax check (all *.toml files)",
            cmd: vec![
                "sh", "-c",
                "find . -type f -name '*.toml' -not -path './plugin_templates/*' -not -path './mcp-servers/templates/*' -print0 | xargs -0 -I{} tomlcheck \"{}\"",
            ],
            only_when_staged: None,
            advisory: false,
            group: Group::All,
        },
        CheckDef {
            name: "license-check",
            description: "License policy check (advisory)",
            cmd: vec![
                "python", "scripts/license_checker.py",
                "--config", "license-policy.toml",
                "--report-json", "license-check-report.json",
            ],
            only_when_staged: None,
            advisory: true,
            group: Group::All,
        },
    ]
}
