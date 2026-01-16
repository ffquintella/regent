# Contributing to Regent

We love contributions! Whether you're fixing bugs, adding features, or improving documentation, your help is appreciated.

## Getting Started

### Prerequisites

- Rust 1.70+ ([Install Rust](https://rustup.rs/))
- Git
- Basic knowledge of Puppet or DevOps

### Setting Up Development Environment

1. **Clone the repository**
   ```bash
   git clone https://github.com/ffquintella/regent.git
   cd regent
   ```

2. **Build from source**
   ```bash
   cargo build
   ```

3. **Run tests**
   ```bash
   cargo test
   ```

4. **Format code**
   ```bash
   cargo fmt
   ```

5. **Check for issues**
   ```bash
   cargo clippy
   ```

## Development Workflow

### 1. Create a Feature Branch

```bash
git checkout -b feature/your-feature-name
# or for bug fixes
git checkout -b fix/your-bug-fix
```

### 2. Make Your Changes

- Write code following Rust conventions
- Add tests for new functionality
- Update documentation if needed

### 3. Test Thoroughly

```bash
# Run all tests
cargo test

# Run specific test
cargo test your_test_name

# Test with verbose output
cargo test -- --nocapture

# Run benchmarks
cargo bench
```

### 4. Format and Lint

```bash
# Format code
cargo fmt

# Check for issues
cargo clippy -- -D warnings

# Check for security issues
cargo audit
```

### 5. Commit Your Changes

```bash
git add .
git commit -m "descriptive commit message"
```

Use conventional commits:
- `feat:` for new features
- `fix:` for bug fixes
- `docs:` for documentation
- `test:` for test improvements
- `refactor:` for code refactoring
- `perf:` for performance improvements
- `chore:` for maintenance

Example:
```bash
git commit -m "feat: add support for puppet 8.x"
git commit -m "fix: validate metadata.json correctly"
```

### 6. Push to Your Fork

```bash
git push origin feature/your-feature-name
```

### 7. Create a Pull Request

1. Go to [GitHub](https://github.com/ffquintella/regent)
2. Click "New Pull Request"
3. Select your branch
4. Fill in the PR template
5. Submit

## Code Guidelines

### Rust Code Style

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use meaningful variable names
- Add comments for complex logic
- Keep functions small and focused

### Example

```rust
/// Validates a Puppet module at the specified path
/// 
/// # Arguments
/// * `path` - Path to the module to validate
/// 
/// # Returns
/// * `Ok(())` if validation passes
/// * `Err` with validation errors
pub fn validate_module(path: &Path) -> anyhow::Result<()> {
    // Check if path exists
    if !path.exists() {
        return Err(anyhow::anyhow!("Module path not found: {:?}", path));
    }

    // Validate structure
    validate_structure(path)?;
    validate_metadata(path)?;

    Ok(())
}
```

### Ruby Code Style (via Artichoke)

- Follow [Ruby Style Guide](https://rubystyle.guide/)
- Use descriptive method names
- Add documentation comments

```ruby
# Example Ruby code for Regent
def validate_module(path)
  raise "Module path not found" unless File.exist?(path)
  
  validate_structure(path)
  validate_metadata(path)
  
  true
end
```

## Working with Features

### Adding a New CLI Command

1. Create a new file in `src/cli/`
2. Implement the command handler
3. Add it to `src/main.rs` in the `Commands` enum
4. Write tests

Example:
```rust
// src/cli/my_command.rs
pub struct MyCommand;

impl MyCommand {
    pub fn execute() -> anyhow::Result<()> {
        // Your implementation
        Ok(())
    }
}
```

### Adding Artichoke Ruby Integration

1. Extend `src/ruby_interop.rs`
2. Add Ruby FFI functions as needed
3. Test Rust-Ruby calling

```rust
impl RubyEnvironment {
    pub fn my_new_function(&self, param: String) -> anyhow::Result<String> {
        // Implementation
        Ok(result)
    }
}
```

### Adding Tests

Place tests near the code they test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_module_success() {
        // Setup
        let test_module = create_test_module();
        
        // Execute
        let result = validate_module(&test_module);
        
        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_module_missing_metadata() {
        let test_module = create_test_module_without_metadata();
        let result = validate_module(&test_module);
        
        assert!(result.is_err());
    }
}
```

## Documentation

### Updating README.md

Keep the README current with:
- New features
- Updated installation instructions
- New examples

### Adding Code Documentation

Document public APIs:

```rust
/// Processes a Puppet module
/// 
/// # Examples
/// ```
/// let result = process_module(Path::new("/path/to/module"))?;
/// println!("Processed: {:?}", result);
/// ```
pub fn process_module(path: &Path) -> anyhow::Result<ProcessResult> {
    // ...
}
```

### Writing Documentation Files

Create `.md` files in the root:
- `FEATURES.md` - Feature documentation
- `TUTORIAL.md` - Step-by-step tutorials
- `FAQ.md` - Frequently asked questions

## Reporting Issues

### Bug Reports

Include:
- OS and Regent version
- Steps to reproduce
- Expected behavior
- Actual behavior
- Error logs

### Feature Requests

Include:
- Use case/problem you're solving
- Proposed solution
- Alternative approaches considered
- Examples if applicable

## Review Process

Your PR will be reviewed by maintainers:

1. **Automated checks** - CI/CD pipeline runs
2. **Code review** - Maintainers review your code
3. **Feedback** - Request changes if needed
4. **Approval** - Once approved, your code is merged

## Project Structure

```
regent/
├── src/
│   ├── main.rs              # CLI entry point
│   ├── lib.rs               # Library root
│   ├── cli/                 # CLI commands
│   ├── config.rs            # Configuration
│   ├── generator.rs         # Generation logic
│   ├── validator.rs         # Validation
│   ├── builder.rs           # Building
│   ├── tester.rs            # Testing
│   ├── ruby_interop.rs      # Ruby/Rust bridge
│   └── artichoke_runtime.rs # Artichoke config
├── lib/                     # Ruby library
├── templates/               # Code templates
├── spec/                    # Ruby tests
├── tests/                   # Integration tests
├── Cargo.toml              # Rust dependencies
├── Gemfile                 # Ruby dependencies
└── README.md               # Documentation
```

## Release Process

Releases follow [semantic versioning](https://semver.org/):

- **MAJOR**: Breaking changes
- **MINOR**: New features
- **PATCH**: Bug fixes

Example: `1.2.3`

## Questions?

- 📖 Check [documentation](./README.md)
- 💬 Open a [discussion](https://github.com/ffquintella/regent/discussions)
- 🐛 File an [issue](https://github.com/ffquintella/regent/issues)

## License

By contributing, you agree that your contributions will be licensed under the AGPL-3.0 License.

---

Thank you for contributing to Regent! 🎉
