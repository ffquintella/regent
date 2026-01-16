# Changelog

All notable changes to the Regent VS Code extension will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-01-16

### Added
- Initial release of Regent VS Code extension
- Commands for build, test, lint, generate, and validators
- Status bar integration with quick menu
- Real-time status indicators (spinning, success, error icons)
- Problem matcher integration for lint and test output
- Diagnostics panel integration for lint issues
- Code action provider with quick-fix suggestions
- Auto-fix command for resolvable lint issues
- Workspace setup command (creates tasks.json and settings.json)
- Lint-on-save functionality (configurable)
- 12 Puppet and RSpec snippets
- Configuration options:
  - Binary path
  - Puppet/Ruby version hints
  - Lint on save toggle
  - Fail on warnings
  - Enable/disable diagnostics
- Task templates for VS Code Tasks panel
- Launch configurations for extension development
- Comprehensive README with usage instructions
- EXAMPLES.md with practical workflows
- Maestro-themed extension icon
- Integration test suite
- Error handling with helpful messages
- Performance optimizations with diagnostic caching
- AGPL-3.0 license

### Features Overview
- **Telemetry-free**: No tracking or analytics
- **Offline-friendly**: Works without network connectivity
- **Lightweight**: Minimal dependencies
- **Fast**: Rust-powered Regent CLI integration

### Requirements
- Regent CLI installed and in PATH
- VS Code 1.85.0 or higher
- Puppet module with metadata.json

[0.1.0]: https://github.com/ffquintella/regent/releases/tag/v0.1.0
