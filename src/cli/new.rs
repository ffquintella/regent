use colored::*;
use std::fs;
use std::path::Path;

pub struct NewCommand;

impl NewCommand {
    pub fn execute(
        name: &str,
        author: Option<&str>,
        license: &str,
        description: Option<&str>,
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
        let metadata = Self::generate_metadata(name, author, license, description);
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
    ) -> String {
        serde_json::to_string_pretty(&serde_json::json!({
            "name": name,
            "version": "0.1.0",
            "author": author.unwrap_or("Unknown"),
            "license": license,
            "description": description.unwrap_or("A Puppet module"),
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
