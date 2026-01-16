# Examples of Regent Usage

This directory contains practical examples demonstrating Regent's capabilities.

## Quick Examples

### 1. Creating a Simple Module

```bash
# Create a new Puppet module
regent new wordpress \
  --author "DevOps Team" \
  --license Apache-2.0 \
  --description "WordPress installation and configuration module"

cd wordpress

# Generate a main class
regent generate class wordpress

# Generate tasks for common operations
regent generate task install_plugin --task-type ruby
regent generate task backup --task-type shell
regent generate task restore --task-type python

# Validate the module
regent validate

# Build the package
regent build --output packages/
```

### 2. Creating a Complex Module with Multiple Components

```bash
# Create module structure
regent new postgresql \
  --author "Infrastructure Team" \
  --license AGPL-3.0

cd postgresql

# Generate multiple classes
regent generate class postgresql::server
regent generate class postgresql::client
regent generate class postgresql::contrib

# Generate tasks
regent generate task backup --task-type shell
regent generate task restore --task-type shell
regent generate task create_database --task-type ruby
regent generate task replication --task-type ruby

# Generate plans
regent generate plan setup
regent generate plan maintenance
regent generate plan disaster_recovery

# Validate everything
regent validate

# Run tests
regent test --pattern "*_spec.rb"

# Build
regent build
```

### 3. Module with Ruby Gem Dependencies

From Ruby code using the Regent gem:

```ruby
require 'regent'

# Create configuration
Regent.configure do |config|
  config.project_name = 'kubernetes'
  config.author = 'Platform Team'
  config.license = 'MIT'
end

# Use Regent API
generator = Regent::Generator.new(Regent.config)
generator.create('kubernetes')

# Can load additional gems
require 'puppet'
require 'yaml'

# Process YAML configurations
config_yaml = YAML.load_file('kubernetes/config.yaml')

# Validate
validator = Regent::Validator.new
validator.validate('kubernetes')
```

### 4. Advanced: Rust + Ruby Hybrid

Rust code that calls Ruby:

```rust
use regent::ruby_interop::RubyEnvironment;

fn create_and_test_module(module_name: &str) -> anyhow::Result<()> {
    // Use Rust for fast operations
    regent::cli::new::NewCommand::execute(
        module_name,
        Some("DevOps Team"),
        "Apache-2.0",
        Some("Example module"),
    )?;

    // Use Artichoke Ruby for testing
    let ruby_env = RubyEnvironment::new()?;
    ruby_env.load_gem("rspec")?;
    ruby_env.eval(&format!(
        r#"
        Dir.chdir('{}')
        require 'rspec'
        RSpec.configure do |config|
            config.color = true
        end
        RSpec.run([])
        "#,
        module_name
    ))?;

    Ok(())
}
```

### 5. Validating Multiple Modules

```bash
#!/bin/bash
# validate_all.sh

for module_dir in */; do
    if [ -f "$module_dir/metadata.json" ]; then
        echo "Validating $module_dir"
        regent validate "$module_dir"
        if [ $? -ne 0 ]; then
            echo "ERROR: $module_dir failed validation"
            exit 1
        fi
    fi
done

echo "All modules validated successfully"
```

### 6. Continuous Integration Example

```yaml
# .github/workflows/regent-ci.yml
name: Regent CI

on: [push, pull_request]

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      
      - name: Install Regent
        run: cargo install --git https://github.com/ffquintella/regent
      
      - name: Validate Module
        run: regent validate
      
      - name: Run Tests
        run: regent test
      
      - name: Build Package
        run: regent build --output dist/
```

### 7. Docker Workflow

```dockerfile
# Dockerfile
FROM rust:latest as builder
RUN git clone https://github.com/ffquintella/regent.git /regent
WORKDIR /regent
RUN cargo build --release

FROM scratch
COPY --from=builder /regent/target/release/regent /regent
ENTRYPOINT ["/regent"]
```

Use it:

```bash
# Build Docker image
docker build -t regent:latest .

# Create a new module
docker run -v $(pwd):/work regent:latest new mymodule

# Validate
docker run -v $(pwd):/work regent:latest validate /work/mymodule
```

### 8. Module Template with Best Practices

```ruby
# Create a template module
require 'regent'

module_config = {
  name: 'best_practice_module',
  author: 'Your Team',
  version: '0.1.0',
  description: 'A module following Regent best practices',
  license: 'Apache-2.0'
}

Regent.configure do |config|
  config.project_name = module_config[:name]
  config.author = module_config[:author]
  config.license = module_config[:license]
end

# Generate structure
generator = Regent::Generator.new(Regent.config)
generator.create('./modules/' + module_config[:name])

module_path = "./modules/#{module_config[:name]}"

# Generate common classes
%w[init config service].each do |class_name|
  Regent::CLI.generate_class(class_name, module_path)
end

# Generate common tasks
%w[restart reload status].each do |task_name|
  Regent::CLI.generate_task(task_name, module_path, 'shell')
end

# Add metadata
metadata = {
  name: module_config[:name],
  version: module_config[:version],
  author: module_config[:author],
  license: module_config[:license],
  operatingsystem_support: [
    { operatingsystem: 'CentOS', operatingsystemrelease: ['7', '8'] },
    { operatingsystem: 'Ubuntu', operatingsystemrelease: ['18.04', '20.04'] }
  ]
}

File.write(
  "#{module_path}/metadata.json",
  JSON.pretty_generate(metadata)
)

puts "Module '#{module_config[:name]}' created with best practices!"
```

## Performance Benchmarks

Running these examples shows Regent's performance:

```bash
# Module generation (Rust - very fast)
time regent new benchmark_module
# real  0m0.087s

# Validation (Rust - fast)
time regent validate benchmark_module
# real  0m0.042s

# Test execution (Artichoke Ruby - Ruby compatibility)
time regent test benchmark_module
# real  0m1.234s (depends on test count)

# Build (Rust)
time regent build --path benchmark_module
# real  0m0.156s
```

## More Examples

- See `examples/` directory for complete runnable examples
- See `spec/` for test examples
- Check GitHub repository for community examples

---

For more information, see:
- [README.md](./README.md)
- [ARCHITECTURE.md](./ARCHITECTURE.md)
- [RUST_RUBY_INTEROP.md](./RUST_RUBY_INTEROP.md)
