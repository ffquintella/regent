# Regent

Regent is an alternative OpenVox/Puppet development kit designed to replace the now paid PDK (Puppet Development Kit). It provides a comprehensive framework for developing, testing, and building Puppet modules.

## Features

- **Module Generation**: Scaffold new Puppet modules with proper structure
- **Component Generators**: Create classes, tasks, and plans
- **Validation**: Syntax checking and metadata validation
- **Testing**: Integrated test framework support (RSpec)
- **Building**: Package modules for distribution
- **CLI Interface**: Easy-to-use command-line tools

## Installation

Add this line to your application's Gemfile:

```ruby
gem 'regent'
```

And then execute:

```bash
$ bundle install
```

Or install it yourself as:

```bash
$ gem install regent
```

## Usage

### Creating a New Module

Generate a new Puppet module:

```bash
$ regent new mymodule --author "Your Name" --license Apache-2.0
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
├── spec/
│   └── spec_helper.rb
├── metadata.json
└── README.md
```

### Generating Components

Generate a new class:

```bash
$ cd mymodule
$ regent generate class myclass
```

Generate a new task:

```bash
$ regent generate task mytask
```

### Validating a Module

Validate syntax and structure:

```bash
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
