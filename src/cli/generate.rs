use colored::*;
use std::fs;
use std::path::{Path, PathBuf};

pub struct GenerateCommand;

impl GenerateCommand {
    pub fn class(name: &str, module_path: &Path) -> anyhow::Result<()> {
        let manifests_path = module_path.join("manifests");
        if !manifests_path.exists() {
            return Err(anyhow::anyhow!("Manifests directory not found"));
        }

        // Resolve the requested name into a fully-qualified Puppet class name
        // and the autoload-correct manifest path (relative to manifests/).
        let module_namespace = module_namespace(module_path);
        let class_name = fully_qualified_class_name(name, &module_namespace);
        let relative_file = manifest_path_for_class(&class_name, &module_namespace);
        let class_file = manifests_path.join(&relative_file);

        // Nested classes (e.g. `mymodule::foo::bar`) map onto subdirectories,
        // so ensure the parent directory exists before writing.
        if let Some(parent) = class_file.parent() {
            fs::create_dir_all(parent)?;
        }

        let class_content = format!(
            "# @summary A short summary of the purpose of this class\n#\n# A description of what this class does\n#\n# @example\n#   include {class_name}\nclass {class_name} (\n) {{\n  # Your class code here\n}}\n",
        );

        fs::write(&class_file, class_content)?;
        println!(
            "{} Generated class: {} ({})",
            "✓".green(),
            class_name,
            relative_file.display()
        );

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

/// Determines the module's Puppet namespace (its short name).
///
/// Prefers the `name` field of `metadata.json` (Forge names look like
/// `author-modname`, so the namespace is the segment after the last `-` or
/// `/`). Falls back to the module directory's basename when metadata is
/// missing or unreadable.
fn module_namespace(module_path: &Path) -> String {
    let from_metadata = fs::read_to_string(module_path.join("metadata.json"))
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|meta| {
            meta.get("name")
                .and_then(|v| v.as_str())
                .map(short_module_name)
        })
        .filter(|s| !s.is_empty());

    from_metadata
        .or_else(|| {
            module_path
                .canonicalize()
                .ok()
                .as_deref()
                .and_then(Path::file_name)
                .and_then(|n| n.to_str())
                .map(short_module_name)
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "mymodule".to_string())
}

/// Extracts the module short name from a Forge-style `author-modname` (or
/// `author/modname`) identifier.
fn short_module_name(full: &str) -> String {
    full.rsplit(['-', '/']).next().unwrap_or(full).to_string()
}

/// Normalizes a user-supplied class name into a fully-qualified Puppet class
/// name, prefixing the module namespace when the caller gave a bare name.
fn fully_qualified_class_name(name: &str, module_namespace: &str) -> String {
    let name = name.trim_start_matches("::");
    if name == module_namespace || name.starts_with(&format!("{module_namespace}::")) {
        name.to_string()
    } else {
        format!("{module_namespace}::{name}")
    }
}

/// Maps a fully-qualified class name to its autoload path relative to
/// `manifests/`. The main class (`modname`) lives in `init.pp`; every other
/// class drops the leading namespace segment and maps `::` onto directories.
fn manifest_path_for_class(class_name: &str, module_namespace: &str) -> PathBuf {
    let mut segments = class_name.split("::");
    // Drop the leading module namespace segment when present.
    if segments.clone().next() == Some(module_namespace) {
        segments.next();
    }
    let rest: Vec<&str> = segments.collect();
    if rest.is_empty() {
        PathBuf::from("init.pp")
    } else {
        let mut path = PathBuf::new();
        for segment in &rest[..rest.len() - 1] {
            path.push(segment);
        }
        path.push(format!("{}.pp", rest[rest.len() - 1]));
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn short_module_name_strips_forge_author() {
        assert_eq!(short_module_name("felipe-mymodule"), "mymodule");
        assert_eq!(short_module_name("puppetlabs/apache"), "apache");
        assert_eq!(short_module_name("mymodule"), "mymodule");
    }

    #[test]
    fn bare_name_is_prefixed_with_namespace() {
        assert_eq!(
            fully_qualified_class_name("config", "mymodule"),
            "mymodule::config"
        );
    }

    #[test]
    fn already_qualified_name_is_left_alone() {
        assert_eq!(
            fully_qualified_class_name("mymodule::config", "mymodule"),
            "mymodule::config"
        );
        assert_eq!(
            fully_qualified_class_name("::mymodule::config", "mymodule"),
            "mymodule::config"
        );
    }

    #[test]
    fn module_name_itself_maps_to_main_class() {
        assert_eq!(fully_qualified_class_name("mymodule", "mymodule"), "mymodule");
        assert_eq!(
            manifest_path_for_class("mymodule", "mymodule"),
            PathBuf::from("init.pp")
        );
    }

    #[test]
    fn class_maps_to_autoload_path() {
        assert_eq!(
            manifest_path_for_class("mymodule::config", "mymodule"),
            PathBuf::from("config.pp")
        );
        assert_eq!(
            manifest_path_for_class("mymodule::foo::bar", "mymodule"),
            PathBuf::from("foo").join("bar.pp")
        );
    }

    fn module_with_metadata(name: &str) -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("manifests")).unwrap();
        fs::write(
            dir.path().join("metadata.json"),
            format!(r#"{{"name":"{name}"}}"#),
        )
        .unwrap();
        dir
    }

    #[test]
    fn generate_class_writes_autoload_correct_file_and_body() {
        let dir = module_with_metadata("felipe-mymodule");
        GenerateCommand::class("config", dir.path()).unwrap();

        let path = dir.path().join("manifests/config.pp");
        assert!(path.exists(), "expected manifests/config.pp to exist");
        let body = fs::read_to_string(&path).unwrap();
        assert!(
            body.contains("class mymodule::config ("),
            "class name should not be doubled up, got:\n{body}"
        );
        assert!(!body.contains("mymodule::config::mymodule"));
        assert!(body.contains("include mymodule::config"));
    }

    #[test]
    fn generate_nested_class_creates_subdirectories() {
        let dir = module_with_metadata("felipe-mymodule");
        GenerateCommand::class("server::config", dir.path()).unwrap();

        let path = dir.path().join("manifests/server/config.pp");
        assert!(path.exists(), "expected manifests/server/config.pp to exist");
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains("class mymodule::server::config ("));
    }

    #[test]
    fn generate_fully_qualified_name_is_not_re_prefixed() {
        let dir = module_with_metadata("felipe-mymodule");
        GenerateCommand::class("mymodule::config", dir.path()).unwrap();

        assert!(dir.path().join("manifests/config.pp").exists());
        assert!(!dir.path().join("manifests/mymodule::config.pp").exists());
    }
}
