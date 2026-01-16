# Regent - OpenVox Development Kit for VS Code

A lightweight, Rust-powered extension for Puppet module development. Build, test, and validate Puppet modules with speed and simplicity.

## Features

### 🚀 Fast Commands

- **Build**: Package your Puppet module with `Regent: Build Module`
- **Test**: Run RSpec tests with `Regent: Run Tests`  
- **Lint**: Validate code with `Regent: Lint Module`
- **Generate**: Scaffold new components with `Regent: Generate Component`

### 🔍 Integrated Validation

- Real-time diagnostics in the Problems panel
- Lint-on-save support (optional)
- Multiple output formats (JSON, text, HTML)

### 📝 Smart Snippets

Pre-configured snippets for:
- Classes and defined types with documentation
- Common resources (file, package, service, exec)
- RSpec test blocks (describe, context, it)
- Ruby functions and Hiera lookups

### ⚙️ Configurable

- Custom Regent binary path
- Puppet/Ruby version hints
- Fail-on-warnings mode
- Enable/disable diagnostics

## Requirements

- [Regent CLI](https://github.com/regent/regent) installed and in PATH
- Puppet module with `metadata.json` in workspace

## Extension Settings

This extension contributes the following settings:

- `regent.binaryPath`: Path to the Regent binary (default: `regent`)
- `regent.puppetVersion`: Puppet version hint for validation (e.g., `7.x`, `8.x`)
- `regent.rubyVersion`: Ruby version hint for validation (e.g., `3.1`, `3.2`)
- `regent.lintOnSave`: Run lint automatically when saving Puppet files (default: `false`)
- `regent.failOnWarnings`: Treat lint warnings as errors (default: `false`)
- `regent.enableDiagnostics`: Show lint issues in Problems panel (default: `true`)

## Usage

### Command Palette

Open the Command Palette (`Cmd+Shift+P` / `Ctrl+Shift+P`) and type:

- `Regent: Build Module` - Package module into tarball
- `Regent: Run Tests` - Execute RSpec test suite
- `Regent: Lint Module` - Validate code with all linters
- `Regent: List Validators` - Show available validation tools
- `Regent: Generate Component` - Scaffold new class, type, resource, etc.

### Snippets

In any `.pp` file, type:

- `regent-class` - Create a documented Puppet class
- `regent-define` - Create a defined type
- `regent-file` - Add a file resource
- `regent-package` - Add a package resource
- `regent-service` - Add a service resource

In RSpec files (`.rb`):

- `regent-describe` - Create a describe block with OS matrix
- `regent-context` - Add a test context
- `regent-it` - Add an expectation

### Tasks

Pre-configured tasks available via `Tasks: Run Task`:

- **Regent: Build Module** (default build task)
- **Regent: Run Tests** (default test task)
- **Regent: Lint Module**
- **Regent: Lint (JSON output)**

## Development

### Building the Extension

```bash
cd vscode-extension
npm install
npm run compile
```

### Running in Development

1. Open the `vscode-extension` folder in VS Code
2. Press `F5` to launch Extension Development Host
3. Test commands in the new VS Code window

### Packaging

```bash
npm run package
```

This creates a `.vsix` file for distribution.

## Design Philosophy

- **Telemetry-free**: No tracking, no analytics
- **Offline-friendly**: Works without network connectivity
- **Lightweight**: Minimal dependencies, fast startup
- **Rust-powered**: Leverages Regent's speed and reliability

## License

MIT

## Links

- [GitHub Repository](https://github.com/ffquintella/regent)
- [Examples and Workflows](EXAMPLES.md)
- [Report Issues](https://github.com/ffquintella/regent/issues)

## Contributing

Issues and pull requests welcome at [regent/regent-vscode](https://github.com/regent/regent-vscode)
