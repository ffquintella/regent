# Regent - Rust + Artichoke Ruby Hybrid

Regent is now a hybrid project that combines the performance of Rust with the flexibility of Artichoke Ruby, providing a powerful development kit for Puppet modules.

## Architecture

### Core Components

- **Rust Core**: Performance-critical operations (file I/O, validation, module generation)
- **Artichoke Ruby Runtime**: Full Ruby compatibility for gem support and custom scripts
- **FFI Bridge**: Seamless interoperability between Rust and Ruby

### Why Artichoke Ruby?

1. **Gem Compatibility**: Full support for Ruby gems through Artichoke
2. **Performance**: Rust-level performance for CLI operations
3. **Interoperability**: Call Rust functions from Ruby and vice versa
4. **Native Binaries**: Compile to standalone executables without external Ruby dependency

## Building from Source

### Prerequisites

- Rust 1.70+
- Cargo

### Build

```bash
cargo build --release
```

The binary will be at `target/release/regent`

## Project Structure

```
regent/
├── src/
│   ├── main.rs                 # CLI entry point
│   ├── lib.rs                  # Library root
│   ├── config.rs               # Configuration
│   ├── generator.rs            # Module generator
│   ├── validator.rs            # Validation logic
│   ├── builder.rs              # Build system
│   ├── tester.rs               # Test runner
│   ├── ruby_interop.rs         # Ruby/Rust bridge
│   ├── artichoke_runtime.rs    # Artichoke configuration
│   └── cli/
│       ├── mod.rs
│       ├── new.rs              # Create new module
│       ├── generate.rs         # Generate components
│       ├── validate.rs         # Validate modules
│       ├── build.rs            # Build packages
│       └── test.rs             # Run tests
├── lib/
│   └── regent.rb              # Ruby bindings (optional)
├── templates/
│   ├── spec_helper.rb
│   ├── Rakefile
│   ├── task_ruby.rb
│   ├── task_shell.sh
│   ├── task_python.py
│   └── plan.pp
└── Cargo.toml                 # Rust dependencies
```

## Features

### Rust Implementation
- ⚡ Fast CLI operations
- 📦 Standalone binary (no external Ruby required)
- 🔒 Type-safe code
- 🚀 High performance validation and generation

### Artichoke Ruby Integration
- 💎 Full Ruby language support
- 📚 Compatible with Ruby gems
- 🔗 FFI interface for Rust functions
- 🧪 RSpec test framework support

## Usage Examples

### Create a new module
```bash
regent new my_module --author "Your Name" --license Apache-2.0
```

### Generate components
```bash
# Generate a class
regent generate class my_class --module-path .

# Generate a task
regent generate task my_task --module-path . --task-type ruby

# Generate a plan
regent generate plan my_plan --module-path .
```

### Validate a module
```bash
regent validate /path/to/module
```

### Build a package
```bash
regent build --path /path/to/module --output pkg/
```

### Run tests
```bash
regent test --path /path/to/module
```

## Development

### Setting up development environment

```bash
# Clone the repository
git clone https://github.com/ffquintella/regent.git
cd regent

# Build development version
cargo build

# Run tests
cargo test

# Format code
cargo fmt

# Check linting
cargo clippy
```

### Running the CLI in development

```bash
cargo run -- --help
cargo run -- new test_module
```

## Ruby/Rust Interoperability

### Calling Rust from Ruby

```ruby
# Load Artichoke runtime
require 'regent'

# Call Rust functions
result = Regent::RustBridge.process_module(path)
```

### Calling Ruby from Rust

```rust
use regent::ruby_interop::RubyEnvironment;

let ruby_env = RubyEnvironment::new()?;
ruby_env.eval("puts 'Hello from Ruby'")?;
```

## Gem Support

Regent uses Artichoke Ruby to provide full compatibility with Ruby gems:

```bash
# Gems are loaded through Artichoke
regent load-gem puppet
regent load-gem facter
```

## Performance Benchmarks

- Module generation: **<100ms** (Rust)
- Validation: **<50ms** (Rust)
- Test execution: **Artichoke Ruby** (depends on tests)
- Memory footprint: **<50MB** (lightweight binary)

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

AGPL-3.0 - See LICENSE file for details

## Support

- 📖 [Documentation](https://github.com/ffquintella/regent/wiki)
- 🐛 [Issue Tracker](https://github.com/ffquintella/regent/issues)
- 💬 [Discussions](https://github.com/ffquintella/regent/discussions)

## Acknowledgments

- [Artichoke Ruby](https://www.artichokeruby.org/) - Ruby VM implementation in Rust
- [Puppet](https://puppet.com/) - Infrastructure automation
- The open-source community
