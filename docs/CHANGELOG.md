# Regent Changelog

## [Unreleased]

## [0.9.4]

### Fixed
- Parser: official Forge modules (puppetlabs/docker 10.5.0, apt 11.4.0, stdlib 10.1.0) now parse completely. Added: selector over a parenthesised comparison (`($a == '7') ? { … }`), `if`/`elsif`/`else` and `unless` as a value (`$x = if … { … } else { … }`), comma-separated string lists in `case` branches, bare `key => value` named arguments in function calls (`epp('t', a => $a)`), `assert_type(Type, $v) |$a, $b| { … }`, trailing comma after `create_resources(…)` arguments, and `unless`/`if` used as attribute names.
- Parse errors are reported as `file:line:col: error: near \`source line\`` instead of a raw byte offset, and the skipped-fixture warning keeps the underlying cause.
- Evaluator: `assert_type` with a lambda was silently ignored (its `fail()` always ran, which looked like missing facts); `fail("… ${var} …")` interpolates and `fail($var)` uses the value; Puppet-language functions under `functions/` (`function ns::name($x) { … }`) are loaded and callable; parameter defaults are bound in declaration order so a default may use an earlier parameter (`$conf = "${root}/x"`); a class first included from inside a define keeps its `$class::var` scope afterwards; `join` treats `undef` as empty; multi-statement lambda bodies (`.map |$k| { $v = …; "…" }`) yield their last expression; `.flatten` as a method call.
- Evaluator: numeric regex captures (`$1`…) after `=~`, plus the functions `versioncmp` (Puppet's algorithm), `match`, `split`, `delete_at`, `regsubst` (scalars and arrays), `strip`/`lstrip`/`rstrip`, `downcase`/`upcase`/`capitalize`, `keys`, `values`, `has_key`, `merge`, `unique`, `sort`, `reverse`, `first`, `last`, `flatten`. Unknown functions still evaluate to `undef`.
- Spec runner: `on_supported_os` facts now include `os.distro.codename` for Debian/Ubuntu releases; `let` values can be used by name inside `it` blocks and matcher arguments (`.with(message: expected_msg)`); `shared_examples` blocks take arguments, plus `include_examples`/`include_context`/`it_behaves_like`/`shared_context`; the `include(...)` matcher is supported in attribute expectations (`with_environments(include('A=1'))`); an undefined shared example or unsupported matcher fails with `unsupported spec construct: …` instead of a bare `NoMethodError`.
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
