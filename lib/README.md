# Regent Ruby Library

This directory contains optional Ruby bindings for Regent, allowing integration with existing Ruby projects and gems.

## Installation

```ruby
gem 'regent'
```

## Usage

### Using as a Ruby Gem

```ruby
require 'regent'

# Configure Regent
Regent.configure do |config|
  config.project_name = 'my_module'
  config.author = 'Your Name'
end

# Generate a new module
generator = Regent::Generator.new
generator.create('path/to/module')

# Validate a module
validator = Regent::Validator.new
validator.validate('path/to/module')

# Build a package
builder = Regent::Builder.new
builder.build('path/to/module')

# Run tests
tester = Regent::Tester.new
tester.test('path/to/module')
```

## Interoperability with Rust

Regent's Rust core can be called from Ruby:

```ruby
require 'regent'

# Access Rust functions via Regent::RustBridge (FFI)
result = Regent::RustBridge.validate_module(path)
```

## Integration with Artichoke Ruby

When Artichoke Ruby is available, Regent provides enhanced compatibility:

```ruby
# Artichoke Ruby stdlib functions work seamlessly
Regent.with_artichoke do
  # Your code here
end
```
