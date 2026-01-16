use anyhow::{Context, Result, anyhow};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A module dependency specification from metadata.json
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ModuleDependency {
    pub name: String,
    #[serde(rename = "version_requirement")]
    pub version_req: String,
}

/// A resolved dependency with full information
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedDependency {
    pub name: String,
    pub version: Version,
    pub dependencies: Vec<String>,
}

/// Dependency graph structure
#[derive(Debug, Clone)]
pub struct DependencyTree {
    pub root: String,
    pub nodes: HashMap<String, Vec<String>>,
}

impl DependencyTree {
    pub fn new(root: String) -> Self {
        Self {
            root,
            nodes: HashMap::new(),
        }
    }

    pub fn add_dependency(&mut self, parent: String, child: String) {
        self.nodes.entry(parent).or_insert_with(Vec::new).push(child);
    }

    pub fn has_cycles(&self) -> bool {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        
        for node in self.nodes.keys() {
            if self.has_cycle_util(node, &mut visited, &mut rec_stack) {
                return true;
            }
        }
        
        false
    }

    fn has_cycle_util(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
    ) -> bool {
        if rec_stack.contains(node) {
            return true;
        }
        
        if visited.contains(node) {
            return false;
        }
        
        visited.insert(node.to_string());
        rec_stack.insert(node.to_string());
        
        if let Some(children) = self.nodes.get(node) {
            for child in children {
                if self.has_cycle_util(child, visited, rec_stack) {
                    return true;
                }
            }
        }
        
        rec_stack.remove(node);
        false
    }
}

/// Resolves and validates module dependencies
pub struct DependencyResolver {
    dependencies: Vec<ModuleDependency>,
}

impl DependencyResolver {
    pub fn new(dependencies: Vec<ModuleDependency>) -> Self {
        Self { dependencies }
    }

    /// Validate all dependency version requirements
    pub fn validate(&self) -> Result<()> {
        for dep in &self.dependencies {
            // Validate version requirement format
            VersionReq::parse(&dep.version_req)
                .with_context(|| format!(
                    "Invalid version requirement '{}' for dependency '{}'",
                    dep.version_req, dep.name
                ))?;
            
            // Validate dependency name format (puppetlabs-stdlib or author-module)
            if !dep.name.contains('-') {
                return Err(anyhow!(
                    "Invalid dependency name '{}': must be in format 'author-module'",
                    dep.name
                ));
            }
        }
        
        Ok(())
    }

    /// Check if a specific version satisfies the dependency requirements
    pub fn check_version_compatible(&self, name: &str, version: &Version) -> Result<bool> {
        for dep in &self.dependencies {
            if dep.name == name {
                let req = VersionReq::parse(&dep.version_req)?;
                return Ok(req.matches(version));
            }
        }
        
        // If dependency not found in our list, it's not required
        Ok(true)
    }

    /// Check all dependencies are compatible (stub for future implementation)
    pub fn check_compatible(&self) -> Result<()> {
        self.validate()?;
        
        // Future: Could fetch actual module versions from Forge and validate
        // For now, just validate syntax
        Ok(())
    }

    /// Build a dependency tree (detects circular dependencies)
    pub fn get_dependency_tree(&self, module_name: &str) -> Result<DependencyTree> {
        let mut tree = DependencyTree::new(module_name.to_string());
        
        // Build tree from direct dependencies
        for dep in &self.dependencies {
            tree.add_dependency(module_name.to_string(), dep.name.clone());
        }
        
        // Check for circular dependencies
        if tree.has_cycles() {
            return Err(anyhow!("Circular dependency detected in module dependencies"));
        }
        
        Ok(tree)
    }

    /// Get all dependencies
    pub fn get_dependencies(&self) -> &[ModuleDependency] {
        &self.dependencies
    }

    /// Resolve dependencies to specific versions (stub for future Forge integration)
    pub fn resolve(&self) -> Result<Vec<ResolvedDependency>> {
        // Future: This would query Puppet Forge to get actual versions
        // For now, return empty list (indicates no resolution needed)
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_validation_valid() {
        let deps = vec![
            ModuleDependency {
                name: "puppetlabs-stdlib".to_string(),
                version_req: ">= 4.0.0, < 9.0.0".to_string(),
            },
            ModuleDependency {
                name: "puppetlabs-concat".to_string(),
                version_req: ">= 5.0.0".to_string(),
            },
        ];
        
        let resolver = DependencyResolver::new(deps);
        assert!(resolver.validate().is_ok());
    }

    #[test]
    fn test_dependency_validation_invalid_version() {
        let deps = vec![ModuleDependency {
            name: "puppetlabs-stdlib".to_string(),
            version_req: "invalid_version".to_string(),
        }];
        
        let resolver = DependencyResolver::new(deps);
        assert!(resolver.validate().is_err());
    }

    #[test]
    fn test_dependency_validation_invalid_name() {
        let deps = vec![ModuleDependency {
            name: "invalidname".to_string(),
            version_req: ">= 1.0.0".to_string(),
        }];
        
        let resolver = DependencyResolver::new(deps);
        let result = resolver.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must be in format"));
    }

    #[test]
    fn test_version_compatibility_check() {
        let deps = vec![ModuleDependency {
            name: "puppetlabs-stdlib".to_string(),
            version_req: ">= 4.0.0, < 9.0.0".to_string(),
        }];
        
        let resolver = DependencyResolver::new(deps);
        
        // Test compatible versions
        assert!(resolver
            .check_version_compatible("puppetlabs-stdlib", &Version::parse("5.0.0").unwrap())
            .unwrap());
        assert!(resolver
            .check_version_compatible("puppetlabs-stdlib", &Version::parse("8.9.0").unwrap())
            .unwrap());
        
        // Test incompatible versions
        assert!(!resolver
            .check_version_compatible("puppetlabs-stdlib", &Version::parse("3.9.0").unwrap())
            .unwrap());
        assert!(!resolver
            .check_version_compatible("puppetlabs-stdlib", &Version::parse("9.0.0").unwrap())
            .unwrap());
    }

    #[test]
    fn test_dependency_tree_creation() {
        let deps = vec![
            ModuleDependency {
                name: "puppetlabs-stdlib".to_string(),
                version_req: ">= 4.0.0".to_string(),
            },
            ModuleDependency {
                name: "puppetlabs-concat".to_string(),
                version_req: ">= 5.0.0".to_string(),
            },
        ];
        
        let resolver = DependencyResolver::new(deps);
        let tree = resolver.get_dependency_tree("mymodule").unwrap();
        
        assert_eq!(tree.root, "mymodule");
        assert_eq!(tree.nodes.get("mymodule").unwrap().len(), 2);
        assert!(tree.nodes.get("mymodule").unwrap().contains(&"puppetlabs-stdlib".to_string()));
        assert!(tree.nodes.get("mymodule").unwrap().contains(&"puppetlabs-concat".to_string()));
    }

    #[test]
    fn test_circular_dependency_detection() {
        let mut tree = DependencyTree::new("module_a".to_string());
        tree.add_dependency("module_a".to_string(), "module_b".to_string());
        tree.add_dependency("module_b".to_string(), "module_c".to_string());
        tree.add_dependency("module_c".to_string(), "module_a".to_string()); // Creates cycle
        
        assert!(tree.has_cycles());
    }

    #[test]
    fn test_no_circular_dependencies() {
        let mut tree = DependencyTree::new("module_a".to_string());
        tree.add_dependency("module_a".to_string(), "module_b".to_string());
        tree.add_dependency("module_a".to_string(), "module_c".to_string());
        tree.add_dependency("module_b".to_string(), "module_d".to_string());
        
        assert!(!tree.has_cycles());
    }

    #[test]
    fn test_get_dependencies() {
        let deps = vec![
            ModuleDependency {
                name: "puppetlabs-stdlib".to_string(),
                version_req: ">= 4.0.0".to_string(),
            },
        ];
        
        let resolver = DependencyResolver::new(deps.clone());
        assert_eq!(resolver.get_dependencies(), &deps);
    }

    #[test]
    fn test_resolve_stub() {
        let deps = vec![ModuleDependency {
            name: "puppetlabs-stdlib".to_string(),
            version_req: ">= 4.0.0".to_string(),
        }];
        
        let resolver = DependencyResolver::new(deps);
        let resolved = resolver.resolve().unwrap();
        
        // Current stub returns empty vec
        assert_eq!(resolved.len(), 0);
    }

    #[test]
    fn test_check_compatible() {
        let deps = vec![ModuleDependency {
            name: "puppetlabs-stdlib".to_string(),
            version_req: ">= 4.0.0, < 9.0.0".to_string(),
        }];
        
        let resolver = DependencyResolver::new(deps);
        assert!(resolver.check_compatible().is_ok());
    }
}
