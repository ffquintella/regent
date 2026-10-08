# Regent Changelog

## [Unreleased]

### Fixed
- Apply resource-like class declaration parameters while evaluating child class bodies, without leaking the tested class's parameters into other classes.
- Resolve Puppet `lookup()` values from Hiera and honor the four-argument form's default when a key is absent.
- Support custom fact specs that require Facter, use `FACTERVERSION` and `Facter::Core::Execution`, and expect mocked method calls with specific arguments.
- Preserve caller variables across child class evaluation, honor negative method expectations, and restore mocked methods between examples, including failed examples.

## [0.1.0] - 2026-01-14

### Added
- Initial framework implementation
- Core module structure with configuration support
- Module generator for creating new Puppet modules
- Class and task generators
- Validation framework for syntax and metadata checking
- Builder for packaging modules
- Test runner framework
- CLI interface with Thor
- Commands: new, generate, validate, build, test
- Support for module scaffolding with proper structure
- Metadata.json generation and validation
- Basic Puppet manifest templates
- RSpec test framework integration
