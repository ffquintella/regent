use tempfile::TempDir;

/// Generates Puppet resource declarations and types
pub struct ResourceGenerator {
    resource_type: String,
    resource_name: String,
    attributes: Vec<(String, String)>,
}

impl ResourceGenerator {
    /// Creates a new ResourceGenerator with the given resource type and name
    pub fn new(resource_type: impl Into<String>, resource_name: impl Into<String>) -> Self {
        Self {
            resource_type: resource_type.into(),
            resource_name: resource_name.into(),
            attributes: Vec::new(),
        }
    }

    /// Adds an attribute to the resource
    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.push((key.into(), value.into()));
        self
    }

    /// Generates the Puppet resource files in a temporary directory
    pub fn generate(&self) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let resource_path = temp_dir.path().join(format!("{}.pp", self.resource_type));
        let resource_content = self.generate_resource_content();
        std::fs::write(resource_path, resource_content).unwrap();
        temp_dir
    }

    /// Generates the Puppet resource content as a string
    pub fn generate_resource_content(&self) -> String {
        let attrs = if self.attributes.is_empty() {
            String::new()
        } else {
            let attr_lines = self
                .attributes
                .iter()
                .map(|(k, v)| format!("  {} => {},", k, v))
                .collect::<Vec<_>>()
                .join("\n");
            format!(",\n{}", attr_lines)
        };

        format!(
            "{} {{ '{}'{}  }}\n",
            self.resource_type, self.resource_name, attrs
        )
    }

    /// Generates test specifications for the resource
    pub fn generate_tests(&self) -> String {
        format!(
            r#"require 'spec_helper'

describe 'resource {}' do
  let(:resource) do
    Puppet::Type.type(:{}).new(name: '{}')
  end

  it 'has required parameter' do
    expect(resource[:name]).to eq('{}')
  end

  it 'compiles' do
    expect {{ resource }}.not_to raise_error
  end
end
"#,
            self.resource_type, self.resource_type, self.resource_name, self.resource_name
        )
    }

    /// Generates documentation for the resource
    pub fn generate_docs(&self) -> String {
        format!(
            "# {} Resource\n\nManages {} resources in Puppet.\n\n## Attributes\n\n{}\n",
            self.resource_type,
            self.resource_type.to_lowercase(),
            if self.attributes.is_empty() {
                "None".to_string()
            } else {
                self.attributes
                    .iter()
                    .map(|(k, v)| format!("- `{}`: {} (default: {})", k, k, v))
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
    fn test_resource_generator_creation() {
        let generator = ResourceGenerator::new("service", "nginx");
        assert_eq!(generator.resource_type, "service");
        assert_eq!(generator.resource_name, "nginx");
    }

    #[test]
    fn test_resource_generator_with_attributes() {
        let generator = ResourceGenerator::new("service", "nginx")
            .with_attribute("ensure", "running")
            .with_attribute("enable", "true");
        assert_eq!(generator.attributes.len(), 2);
    }

    #[test]
    fn test_resource_generator_file_creation() {
        let generator = ResourceGenerator::new("service", "nginx");
        let temp_dir = generator.generate();
        let resource_file = temp_dir.path().join("service.pp");
        assert!(resource_file.exists());
    }

    #[test]
    fn test_resource_generator_content() {
        let generator = ResourceGenerator::new("service", "nginx")
            .with_attribute("ensure", "running")
            .with_attribute("enable", "true");
        let content = generator.generate_resource_content();
        assert!(content.contains("service { 'nginx'"));
        assert!(content.contains("ensure => running"));
        assert!(content.contains("enable => true"));
    }

    #[test]
    fn test_resource_generator_tests() {
        let generator = ResourceGenerator::new("service", "nginx");
        let tests = generator.generate_tests();
        assert!(tests.contains("describe 'resource service'"));
        assert!(tests.contains("'nginx'"));
    }

    #[test]
    fn test_resource_generator_docs() {
        let generator = ResourceGenerator::new("service", "nginx");
        let docs = generator.generate_docs();
        assert!(docs.contains("# service Resource"));
        assert!(docs.contains("Manages service resources"));
    }
}
