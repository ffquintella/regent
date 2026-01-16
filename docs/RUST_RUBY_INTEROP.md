# Rust/Ruby Interoperability Guide

This document explains how to use Rust functions from Ruby and call Ruby from Rust in Regent.

## Table of Contents

1. [Calling Rust from Ruby](#calling-rust-from-ruby)
2. [Calling Ruby from Rust](#calling-ruby-from-rust)
3. [Data Type Conversion](#data-type-conversion)
4. [Performance Considerations](#performance-considerations)
5. [Examples](#examples)

## Calling Rust from Ruby

### Using Regent::RustBridge

Regent provides an FFI bridge for calling Rust functions from Ruby:

```ruby
require 'regent'

# Validate a module (Rust implementation)
result = Regent::RustBridge.validate_module('/path/to/module')

# Process configuration (Rust)
config = Regent::RustBridge.parse_config(metadata_path)

# Faster file operations (Rust)
files = Regent::RustBridge.list_module_files('/path/to/module')
```

### Directly Loading the Rust Library

```ruby
require 'regent'

# The binary already contains compiled Rust code
regent = Regent::Regent.new
regent.configure do |config|
  config.project_name = "my_module"
  config.author = "Your Name"
end
```

## Calling Ruby from Rust

### Using the RubyEnvironment API

In your Rust code:

```rust
use regent::ruby_interop::RubyEnvironment;

fn main() -> anyhow::Result<()> {
    // Initialize Artichoke Ruby environment
    let ruby_env = RubyEnvironment::new()?;

    // Execute Ruby code
    ruby_env.eval(r#"
        puts "Hello from Ruby in Rust"
        result = 42
    "#)?;

    // Call Ruby functions
    let output = ruby_env.call_function("my_ruby_function", vec!["arg1".to_string()])?;
    println!("Ruby returned: {}", output);

    // Load and use gems
    ruby_env.load_gem("puppet")?;

    Ok(())
}
```

### FFI Functions

```rust
use regent::ruby_interop::ffi;

fn my_rust_function() {
    let result = ffi::rust_function_from_ruby("hello".to_string());
    println!("FFI result: {}", result);
}
```

## Data Type Conversion

### Ruby to Rust

| Ruby Type | Rust Type | Conversion |
|-----------|-----------|-----------|
| String | String | `.to_string()` |
| Integer | i64 | `.parse().unwrap()` |
| Float | f64 | `.parse().unwrap()` |
| Array | Vec | parse JSON |
| Hash | HashMap | parse JSON |
| true/false | bool | `.to_string() == "true"` |

### Rust to Ruby

| Rust Type | Ruby Type | Conversion |
|-----------|-----------|-----------|
| String | String | automatic |
| i64 | Integer | automatic |
| f64 | Float | automatic |
| Vec | Array | to JSON |
| HashMap | Hash | to JSON |
| bool | true/false | automatic |

### JSON Bridge

The easiest way to pass complex types between Rust and Ruby is through JSON:

**Rust:**

```rust
use serde_json::json;

let data = json!({
    "name": "my_module",
    "version": "1.0.0",
    "tags": ["puppet", "configuration"]
});

ruby_env.eval(&format!(r#"
    data = {}
    puts data["name"]
"#, data.to_string()))?;
```

**Ruby:**

```ruby
require 'json'

data = {
  name: "my_module",
  version: "1.0.0",
  tags: ["puppet", "configuration"]
}

# This JSON is passed to Rust
Regent::RustBridge.process_module_data(data.to_json)
```

## Performance Considerations

### When to Use Rust

Use Rust for performance-critical operations:

- ✅ File I/O operations
- ✅ Large data processing
- ✅ Validation logic
- ✅ String parsing
- ✅ Batch operations

```rust
// Fast path - use Rust
pub fn validate_puppet_files(path: &Path) -> anyhow::Result<Vec<ValidationError>> {
    // High-performance validation
    let mut errors = Vec::new();
    // ... validation logic
    Ok(errors)
}
```

### When to Use Artichoke Ruby

Use Artichoke Ruby for flexibility and compatibility:

- ✅ Test execution
- ✅ Custom scripts
- ✅ Gem integration
- ✅ Complex business logic
- ✅ Puppet DSL code execution

```ruby
# Artichoke Ruby path - flexible and compatible
def custom_module_setup
  # Can use gems here
  require 'puppet'
  
  # Can use DSL
  Puppet::Resource.new(...)
end
```

### Hybrid Approach (Recommended)

Combine both for optimal performance:

```rust
// src/validator.rs
pub async fn validate_module(path: &Path) -> anyhow::Result<ValidationReport> {
    // Rust: Fast file validation
    let rust_errors = validate_puppet_syntax(path)?;
    
    // Ruby: Run RSpec tests via Artichoke
    let test_results = run_ruby_tests(path)?;
    
    Ok(ValidationReport {
        syntax_errors: rust_errors,
        test_results: test_results,
    })
}
```

## Examples

### Example 1: Validate and Test a Module

```rust
use regent::ruby_interop::RubyEnvironment;
use regent::validator::Validator;

fn validate_and_test(module_path: &Path) -> anyhow::Result<()> {
    // Rust validation (fast)
    Validator::validate(module_path)?;
    println!("✓ Syntax validation passed");

    // Ruby tests via Artichoke (with gems)
    let ruby = RubyEnvironment::new()?;
    ruby.load_gem("rspec")?;
    ruby.eval(&format!(
        r#"
        Dir.chdir('{}')
        RSpec.configure {{ |config| config.color = true }}
        RSpec.run([])
        "#,
        module_path.display()
    ))?;

    Ok(())
}
```

### Example 2: Generate and Validate

```ruby
require 'regent'

# Use Rust for generation (fast)
Regent::RustBridge.generate_class("my_class", "/path/to/module")

# Use Ruby for setup
File.write("/path/to/module/manifests/my_class.pp", 
  "class mymodule::my_class { # Your code }")

# Use Rust for validation
result = Regent::RustBridge.validate_module("/path/to/module")
puts result[:status]
```

### Example 3: Custom Gem Integration

```rust
// Call Ruby code that uses gems
let ruby_env = RubyEnvironment::new()?;
ruby_env.load_gem("puppet")?;
ruby_env.eval(r#"
    # Now puppet gem is available
    require 'puppet/face'
    
    # Use Puppet's functionality
    resources = Puppet::Face[:resource, :current].find_all
    
    # Return results
    resources.to_json
"#)?;
```

## Best Practices

### 1. Keep the Boundary Clean

```rust
// ✓ Good: Clear interface
pub fn validate_module(path: &Path) -> anyhow::Result<ValidationResult> {
    // Rust implementation
}

// ✗ Bad: Too much interop
pub fn validate_with_ruby_fallback(path: &Path) -> anyhow::Result<()> {
    match validate_in_rust(path) {
        Ok(r) => Ok(r),
        Err(_) => ruby_env.eval("validate(path)"), // Complex fallback
    }
}
```

### 2. Batch Operations

```rust
// ✓ Good: Batch processing
pub fn process_modules(paths: Vec<&Path>) -> anyhow::Result<Vec<Result>> {
    paths.par_iter().map(validate_module).collect()
}

// ✗ Bad: Individual calls across boundary
for path in paths {
    ruby_env.call_function("process", vec![path.to_string()])?;
}
```

### 3. Use Types to Document Intent

```rust
// ✓ Good: Clear intent
pub fn ruby_execute_tests(test_dir: &Path) -> anyhow::Result<TestResults> {
    // Obviously runs Ruby tests
}

// ✗ Bad: Unclear what happens
pub fn run_tests(path: &Path) -> anyhow::Result<()> {
    // Is this Rust or Ruby?
}
```

### 4. Error Handling

```rust
// ✓ Good: Clear error sources
pub fn validate_with_ruby_tests(module_path: &Path) -> anyhow::Result<()> {
    Validator::validate(module_path)
        .context("Rust validation failed")?;
    
    let ruby = RubyEnvironment::new()
        .context("Failed to initialize Ruby")?;
    ruby.eval("run_tests()")
        .context("Ruby tests failed")?;
    
    Ok(())
}
```

## Debugging

### Enable Logging

```bash
# In Rust code
RUST_LOG=debug cargo run -- validate

# In Ruby code via Artichoke
require 'logger'
logger = Logger.new(STDERR)
```

### Inspect FFI Calls

```rust
// Add debugging in ruby_interop.rs
pub fn call_function(&self, name: &str, args: Vec<String>) -> anyhow::Result<String> {
    eprintln!("DEBUG: Calling Ruby function: {} with args: {:?}", name, args);
    // ... call implementation
}
```

## Troubleshooting

| Issue | Solution |
|-------|----------|
| "Undefined method" error in Ruby | Check FFI bridge is loaded |
| Type conversion errors | Use JSON for complex types |
| Performance degradation | Profile with both rustflame and Ruby profiler |
| Gem not loading | Ensure gem is Artichoke-compatible |
| Memory leaks | Check Ruby object lifecycle in FFI |

---

For more information, see:
- [ARCHITECTURE.md](./ARCHITECTURE.md)
- [ARTICHOKE_INTEGRATION.md](./ARTICHOKE_INTEGRATION.md)
