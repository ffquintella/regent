use colored::*;
use std::fs;
use std::path::Path;

pub struct GenerateCommand;

impl GenerateCommand {
    pub fn class(name: &str, module_path: &Path) -> anyhow::Result<()> {
        let manifests_path = module_path.join("manifests");
        if !manifests_path.exists() {
            return Err(anyhow::anyhow!("Manifests directory not found"));
        }

        let class_file = format!("{}.pp", name);
        let class_content = format!(
            "# @summary A short summary of the purpose of this class\n#\n# A description of what this class does\n#\n# @example\n#   include {}::{}\nclass {}::{} (\n) {{\n  # Your class code here\n}}\n",
            name, name, name, name
        );

        fs::write(manifests_path.join(&class_file), class_content)?;
        println!("{} Generated class: {}", "✓".green(), name);

        Ok(())
    }

    pub fn task(name: &str, module_path: &Path, task_type: &str) -> anyhow::Result<()> {
        let tasks_path = module_path.join("tasks");
        if !tasks_path.exists() {
            return Err(anyhow::anyhow!("Tasks directory not found"));
        }

        let (file_ext, template) = match task_type {
            "ruby" => ("rb", include_str!("../../templates/task_ruby.rb")),
            "shell" => ("sh", include_str!("../../templates/task_shell.sh")),
            "python" => ("py", include_str!("../../templates/task_python.py")),
            _ => ("rb", include_str!("../../templates/task_ruby.rb")),
        };

        let task_file = format!("{}.{}", name, file_ext);
        fs::write(tasks_path.join(&task_file), template)?;

        // Create JSON metadata for task
        let task_meta = serde_json::json!({
            "description": format!("A {} task", task_type),
            "parameters": {}
        });

        let task_meta_file = format!("{}.json", name);
        fs::write(
            tasks_path.join(&task_meta_file),
            serde_json::to_string_pretty(&task_meta)?,
        )?;

        println!("{} Generated {} task: {}", "✓".green(), task_type, name);

        Ok(())
    }

    pub fn plan(name: &str, module_path: &Path) -> anyhow::Result<()> {
        let plans_path = module_path.join("plans");
        if !plans_path.exists() {
            return Err(anyhow::anyhow!("Plans directory not found"));
        }

        let plan_content = include_str!("../../templates/plan.pp");
        let plan_file = format!("{}.pp", name);

        fs::write(plans_path.join(&plan_file), plan_content)?;
        println!("{} Generated plan: {}", "✓".green(), name);

        Ok(())
    }
}
