use tempfile::TempDir;

/// Generates Puppet classes from specifications
pub struct ClassGenerator {
    class_name: String,
    namespace: String,
    parameters: Vec<String>,
    resources: Vec<String>,
}

impl ClassGenerator {
    /// Creates a new ClassGenerator with the given class name
    pub fn new(class_name: impl Into<String>) -> Self {
        Self {
            class_name: class_name.into(),
            namespace: "mymodule".to_string(),
            parameters: Vec::new(),
            resources: Vec::new(),
        }
    }

    /// Sets the namespace for the class
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = namespace.into();
        self
    }

    /// Adds a parameter to the class
    pub fn with_parameter(mut self, param: impl Into<String>) -> Self {
        self.parameters.push(param.into());
        self
    }

    /// Adds a resource to the class
    pub fn with_resource(mut self, resource: impl Into<String>) -> Self {
        self.resources.push(resource.into());
        self
    }

    /// Generates the Puppet class files in a temporary directory
    pub fn generate(&self) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let class_path = temp_dir.path().join(format!("{}.pp", self.class_name));
        let class_content = self.generate_class_content();
        std::fs::write(class_path, class_content).unwrap();
        temp_dir
    }

    /// Generates the Puppet class content as a string
    pub fn generate_class_content(&self) -> String {
        let params = if self.parameters.is_empty() {
            String::new()
        } else {
            format!("(\n  {},\n) ", self.parameters.join(",\n  "))
        };

        let resources = if self.resources.is_empty() {
            "  # Define resources here".to_string()
        } else {
            self.resources.join("\n  ")
        };

        format!(
            "class {}::{} {}{{\n  {}\n}}\n",
            self.namespace, self.class_name, params, resources
        )
    }

    /// Generates test specifications for the class
    pub fn generate_tests(&self) -> String {
        format!(
            "require 'spec_helper'\n\
            \n\
            describe '{}::{}' do\n  \
              on_supported_os.each do |os, os_facts|\n    \
                context \"on OS\" do\n      \
                  let(:facts) {{ os_facts }}\n\n      \
                  context 'with default parameters' do\n        \
                    it {{ is_expected.to compile }}\n        \
                    it {{ is_expected.to contain_class('{}::{}') }}\n      \
                  end\n    \
                end\n  \
              end\n\
            end\n",
            self.namespace, self.class_name, self.namespace, self.class_name
        )
    }

    /// Generates documentation for the class
    pub fn generate_docs(&self) -> String {
        format!(
            "# {}\n\nA Puppet class for managing {}.\n\n## Parameters\n\n{}\n",
            self.class_name,
            self.class_name.to_lowercase(),
            if self.parameters.is_empty() {
                "None".to_string()
            } else {
                self.parameters
                    .iter()
                    .map(|p| format!("- `{}`: Parameter description", p))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_class_generator_creation() {
        let generator = ClassGenerator::new("TestClass");
        assert_eq!(generator.class_name, "TestClass");
        assert_eq!(generator.namespace, "mymodule");
    }

    #[test]
    fn test_class_generator_with_namespace() {
        let generator = ClassGenerator::new("TestClass").with_namespace("custom");
        assert_eq!(generator.namespace, "custom");
    }

    #[test]
    fn test_class_generator_with_parameters() {
        let generator = ClassGenerator::new("TestClass")
            .with_parameter("param1")
            .with_parameter("param2");
        assert_eq!(generator.parameters.len(), 2);
    }

    #[test]
    fn test_class_generator_file_creation() {
        let generator = ClassGenerator::new("TestClass");
        let temp_dir = generator.generate();
        let class_file = temp_dir.path().join("TestClass.pp");
        assert!(class_file.exists());
    }

    #[test]
    fn test_class_generator_content() {
        let generator = ClassGenerator::new("TestClass")
            .with_namespace("myns")
            .with_parameter("enable")
            .with_parameter("version");
        let content = generator.generate_class_content();
        assert!(content.contains("class myns::TestClass"));
        assert!(content.contains("enable"));
        assert!(content.contains("version"));
    }

    #[test]
    fn test_class_generator_tests() {
        let generator = ClassGenerator::new("TestClass");
        let tests = generator.generate_tests();
        assert!(tests.contains("describe 'mymodule::TestClass'"));
        assert!(tests.contains("is_expected.to compile"));
    }
}
