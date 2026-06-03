use colored::*;
use std::fs;
use std::path::Path;

pub struct NewCommand;

/// Truncate a summary to a safe length for Puppet's metadata schema
/// (the Forge UI prefers <= 144 chars; long descriptions get a single trailing "…").
fn truncate_summary(text: &str) -> String {
    const MAX: usize = 144;
    let trimmed = text.trim();
    if trimmed.chars().count() <= MAX {
        trimmed.to_string()
    } else {
        let mut out: String = trimmed.chars().take(MAX - 1).collect();
        out.push('…');
        out
    }
}

impl NewCommand {
    pub fn execute(
        name: &str,
        author: Option<&str>,
        license: &str,
        description: Option<&str>,
        summary: Option<&str>,
    ) -> anyhow::Result<()> {
        let module_path = Path::new(name);

        if module_path.exists() {
            return Err(anyhow::anyhow!(
                "Module directory '{}' already exists",
                name
            ));
        }

        // Create directory structure
        let dirs = vec![
            "manifests",
            "files",
            "templates",
            "tasks",
            "plans",
            "lib",
            "lib/puppet",
            "lib/puppet/functions",
            "spec",
            "spec/fixtures",
            "spec/fixtures/modules",
            "pkg",
        ];

        for dir in &dirs {
            fs::create_dir_all(module_path.join(dir))?;
            println!("{} {}/{}", "✓".green(), name, dir);
        }

        // Create metadata.json
        let metadata = Self::generate_metadata(name, author, license, description, summary);
        fs::write(module_path.join("metadata.json"), metadata)?;
        println!("{} {}/metadata.json", "✓".green(), name);

        // Create manifests/init.pp
        let init_pp = format!(
            "# @summary A short summary of the purpose of this class\n#\n# A description of what this class does\n#\n# @example\n#   include {}::{}\nclass {}::{} (\n) {{\n  # Your class code here\n}}\n",
            name, name, name, name
        );
        fs::write(module_path.join("manifests/init.pp"), init_pp)?;
        println!("{} {}/manifests/init.pp", "✓".green(), name);

        // Create README.md
        let readme = Self::generate_readme(name, author, license, description);
        fs::write(module_path.join("README.md"), readme)?;
        println!("{} {}/README.md", "✓".green(), name);

        // Create AGENTS.md — agent-facing guidance for working on this module.
        let agents_md = Self::generate_agents_md(name);
        fs::write(module_path.join("AGENTS.md"), agents_md)?;
        println!("{} {}/AGENTS.md", "✓".green(), name);

        // Create CLAUDE.md as a thin pointer to AGENTS.md so both Claude Code
        // and other agents read the same instructions.
        let claude_md = Self::generate_claude_md();
        fs::write(module_path.join("CLAUDE.md"), claude_md)?;
        println!("{} {}/CLAUDE.md", "✓".green(), name);

        // Create spec_helper.rb
        let spec_helper = include_str!("../../templates/spec_helper.rb");
        fs::write(module_path.join("spec/spec_helper.rb"), spec_helper)?;
        println!("{} {}/spec/spec_helper.rb", "✓".green(), name);

        // Create Rakefile
        let rakefile = include_str!("../../templates/Rakefile");
        fs::write(module_path.join("Rakefile"), rakefile)?;
        println!("{} {}/Rakefile", "✓".green(), name);

        // Create .gitignore
        let gitignore = include_str!("../../templates/gitignore");
        fs::write(module_path.join(".gitignore"), gitignore)?;
        println!("{} {}/.gitignore", "✓".green(), name);

        println!(
            "\n{} Module '{}' created successfully!",
            "✓".green().bold(),
            name
        );

        Ok(())
    }

    fn generate_metadata(
        name: &str,
        author: Option<&str>,
        license: &str,
        description: Option<&str>,
        summary: Option<&str>,
    ) -> String {
        let description = description.unwrap_or("A Puppet module");
        // Puppet's metadata schema requires `summary`. Default to the
        // description (truncated to the recommended ~144 chars) when the
        // caller didn't pass one.
        let summary = summary
            .map(|s| s.to_string())
            .unwrap_or_else(|| truncate_summary(description));
        serde_json::to_string_pretty(&serde_json::json!({
            "name": name,
            "version": "0.1.0",
            "author": author.unwrap_or("Unknown"),
            "license": license,
            "summary": summary,
            "description": description,
            "project_page": "",
            "source": "",
            "issues_url": "",
            "dependencies": [],
            "operatingsystem_support": [],
            "requirements": [
                {
                    "name": "puppet",
                    "version_requirement": ">= 6.0.0"
                }
            ]
        }))
        .unwrap_or_default()
    }

    fn generate_agents_md(name: &str) -> String {
        format!(
            r#"# Agent Instructions for `{name}`

These instructions apply to any AI agent (Claude Code, Copilot, Cursor, Aider, etc.)
working on this Puppet module. Human contributors should follow them too.

## What this module is

`{name}` is a Puppet module. The canonical interface is the manifests in
`manifests/`, with supporting Ruby code under `lib/`, templates in `templates/`,
and tests in `spec/`.

## How to work on it agentically

1. **Read first.** Before editing, scan `metadata.json`, `manifests/init.pp`,
   and any existing classes/defines you're about to touch. Match the existing
   style — parameter ordering, data types, lookup patterns.
2. **Small, focused changes.** One concern per change. Don't refactor unrelated
   code while fixing a bug or adding a feature.
3. **Update tests alongside code.** Every new class, defined type, function, or
   fact must ship with an rspec-puppet spec under `spec/`. Update fixtures in
   `spec/fixtures/` when dependencies change.
4. **Keep `metadata.json` honest.** Update `dependencies`,
   `operatingsystem_support`, and `requirements` whenever the module's surface
   area changes. Bump `version` for releases.
5. **Document parameters with puppet-strings tags** (`@param`, `@example`,
   `@summary`) so the README and reference stay generatable.

## Validate and test with Regent — the single source of truth

**Use [Regent](https://github.com/felipe-quintella/regent) for all validation
and testing of this module.** Do not reach for `puppet`, `bundle exec rspec`,
`pdk`, or a host Ruby toolchain. Regent ships a self-contained binary with an
embedded Ruby runner; it is the supported way to lint, parse, and run specs
against this module.

Typical loop:

```sh
regent validate     # parse manifests + metadata.json, lint
regent test        # run rspec-puppet specs through the embedded runner
regent build       # produce a Forge-ready tarball in pkg/
```

If `regent test` reports a missing gem, run `regent bootstrap` — never
`gem install` or `bundle install`. Regent ships every gem it needs.

When a test fails, fix the code or the spec; do not silence the test or skip it
without an explicit reason captured in a comment.

## Pull request checklist for agents

- [ ] `regent validate` is clean.
- [ ] `regent test` passes locally.
- [ ] `metadata.json` reflects new dependencies / OS support.
- [ ] README or reference docs updated for any new public parameter or class.
- [ ] No new dependency on a host Ruby, `bundle`, or `pdk`.

## Out of scope

- Introducing tooling that requires a host Ruby/Bundler install.
- Editing files under `pkg/` by hand — that directory is build output.
- Committing `spec/fixtures/modules/<name>` symlinks or vendored dependencies
  unless they are genuinely required for tests to run under Regent.
"#,
            name = name
        )
    }

    fn generate_claude_md() -> String {
        r#"# Claude Code Instructions

This module's agent instructions live in [AGENTS.md](AGENTS.md). Read that file
before making any changes — it covers conventions, the test/validate workflow,
and the pull-request checklist.

**TL;DR:** use [Regent](https://github.com/felipe-quintella/regent)
(`regent validate`, `regent test`, `regent build`) as the single tool for
validating and testing this module. Do not invoke host `puppet`, `rspec`,
`bundle`, or `pdk`.
"#
        .to_string()
    }

    fn generate_readme(
        name: &str,
        author: Option<&str>,
        license: &str,
        description: Option<&str>,
    ) -> String {
        format!(
            "# {}\n\n## Description\n\n{}\n\n## Usage\n\n```puppet\ninclude {}\n```\n\n## Reference\n\n## Limitations\n\n## Development\n\nAuthor: {}\nLicense: {}\n",
            name,
            description.unwrap_or("A Puppet module"),
            name,
            author.unwrap_or("Unknown"),
            license
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> serde_json::Value {
        serde_json::from_str(json).expect("generated metadata.json must be valid JSON")
    }

    #[test]
    fn generated_metadata_includes_summary_key() {
        let json = NewCommand::generate_metadata("acme-mymod", None, "Apache-2.0", None, None);
        let value = parse(&json);
        assert!(
            value.get("summary").is_some(),
            "summary key is required by Puppet"
        );
        assert!(
            !value["summary"].as_str().unwrap().is_empty(),
            "summary must not be empty"
        );
    }

    #[test]
    fn explicit_summary_is_used_verbatim() {
        let json = NewCommand::generate_metadata(
            "acme-mymod",
            Some("Felipe"),
            "Apache-2.0",
            Some("a longer description"),
            Some("Custom one-liner"),
        );
        let value = parse(&json);
        assert_eq!(value["summary"], "Custom one-liner");
    }

    #[test]
    fn summary_defaults_to_description_when_short_enough() {
        let json = NewCommand::generate_metadata(
            "acme-mymod",
            None,
            "Apache-2.0",
            Some("Manages the acme widget service"),
            None,
        );
        let value = parse(&json);
        assert_eq!(value["summary"], "Manages the acme widget service");
    }

    #[test]
    fn long_descriptions_get_truncated_with_ellipsis() {
        let long = "x".repeat(300);
        let json =
            NewCommand::generate_metadata("acme-mymod", None, "Apache-2.0", Some(&long), None);
        let value = parse(&json);
        let summary = value["summary"].as_str().unwrap();
        assert!(summary.chars().count() <= 144);
        assert!(summary.ends_with('…'));
    }
}
