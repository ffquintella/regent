# Regent - Rust + Artichoke Ruby Hybrid

Regent is a next-generation alternative to the now paid Puppet Development Kit (PDK). Built with Rust for performance and Artichoke Ruby for compatibility, it provides a comprehensive framework for developing, testing, and building Puppet modules.

## ✨ Key Features

- **⚡ Rust Performance**: Lightning-fast CLI operations and module generation
- **💎 Artichoke Ruby**: Full Ruby language support with gem compatibility
- **🎯 Module Generation**: Scaffold new Puppet modules with proper structure
- **🔧 Component Generators**: Create classes, tasks, and plans
- **✅ Validation**: Syntax checking and metadata validation
- **🧪 Testing**: Integrated RSpec test framework support via Artichoke
- **📦 Building**: Package modules for distribution
- **🌉 Interoperability**: Seamless Rust/Ruby FFI for extending functionality
- **📱 Standalone**: Single binary, no external Ruby dependency required

## Installation

### Option 1: Build from Source (Recommended)

Prerequisites: Rust 1.70+

```bash
git clone https://github.com/ffquintella/regent.git
cd regent
cargo build --release
```

The binary will be at `target/release/regent`

### Option 2: Install as Ruby Gem (Includes compiled binary)

Add this line to your application's Gemfile:

```ruby
gem 'regent'
```Quick Start

### Creating a New Module

Generate a new Puppet module:

```bash
regent new mymodule --author "Your Name" --license Apache-2.0 --description "My awesome module"
```

This creates a complete module structure:

```
mymodule/
├── manifests/
│   └── init.pp
├── files/
├── templates/
├── tasks/
├── plans/
├── lib/
│   └── puppet/
│       └── functions/
├── spec/
│   └── spec_helper.rb
├── metadata.json
├── Rakefile
├── .gitignore
└── README.md
```

### Generating Components

Generate a new class:

```bash
regent generate class myclass --module-path .
```

Generate a new task (Ruby):

```bash
regent generate task mytask --module-path . --task-type ruby
```

Generate a task in Shell or Python:

```bash
regent generate task mytask --module-path . --task-type shell
regent generate task mytask --module-path . --task-type python
```

Generate a new plan:

```bash
regent generate plan myplan --module-path .
```

### Validating a Module

Validate syntax and structure:

```bash
regent validate
```

Or validate a specific path:

```bash
regent validate /path/to/module
```

### Building a Module

Package your module:

```bash
regent build
```

Specify output directory:

```bash
regent build --path /path/to/module --output ./dist
```

This creates a `.tar.gz` package in the specified directory.

### Running Tests

Execute RSpec tests (powered by Artichoke Ruby):

```bash
regent test
```

With pattern matching:

```bash
regent test --pattern "*_spec.rb"
```
$ regent validate
```

Or validate a specific path:

```bash
$ regent validate /path/to/module
```

### Building a Module

Package your module:

```bash
$ regent build
```

This creates a `.tar.gz` package in the `pkg/` directory.

### Running Tests

Run all tests:

```bash
$ regent test
```

Run specific test types:

```bash
$ regent test --type unit
$ regent test --type integration
```

### Command Reference

```bash
regent new MODULE_NAME          # Create a new module
regent generate TYPE NAME       # Generate module components
regent validate [PATH]          # Validate module
regent build [PATH]             # Build and package module
regent test [PATH]              # Run tests
regent version                  # Show version
```

## Development

After checking out the repo, run:

```bash
$ bundle install
$ rake spec
```

To install this gem onto your local machine, run:

```bash
$ bundle exec rake install
```

## Contributing

Bug reports and pull requests are welcome on GitHub at https://github.com/ffquintella/regent.

## License

The gem is available as open source under the terms of the [GNU Affero General Public License v3.0](https://opensource.org/licenses/AGPL-3.0).
