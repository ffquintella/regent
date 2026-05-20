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
        assert!(value.get("summary").is_some(), "summary key is required by Puppet");
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
        let json = NewCommand::generate_metadata(
            "acme-mymod",
            None,
            "Apache-2.0",
            Some(&long),
            None,
        );
        let value = parse(&json);
        let summary = value["summary"].as_str().unwrap();
        assert!(summary.chars().count() <= 144);
        assert!(summary.ends_with('…'));
    }
}
