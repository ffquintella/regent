# Artichoke Ruby Integration Guide

## Overview

Regent is integrated with Artichoke Ruby, a Ruby VM implementation written in Rust. This provides several advantages:

1. **Performance**: Ruby code runs at near-native speed through Rust compilation
2. **Compatibility**: Full support for Ruby language features and gems
3. **Interoperability**: Seamless calling between Rust and Ruby code
4. **Distribution**: Single standalone binary with no external Ruby dependency

## Core Principle: Embedded Ruby Only — No Host Ruby

Regent is a self-contained Rust binary with an **embedded Ruby runner (Artichoke)**. It must run on machines that have no `ruby`, `gem`, or `bundle` on PATH.

Rules contributors and AI agents must follow:

- **Do not shell out to host Ruby tooling.** No `Command::new("ruby")`, `Command::new("gem")`, `Command::new("bundle")`, or `Command::new("rspec")` from Rust code paths used during normal operation.
- **Gem dependencies ship with Regent.** Required gems (rspec, rspec-puppet, puppetlabs_spec_helper, etc.) are bundled in a gem cache that's discovered via `REGENT_BUNDLED_GEMS`, packaged alongside the binary (`share/regent/bundled_gems`), or — in dev — found under `assets/bundled_gems` or `vendor/bundle` in the repo.
- **`regent bootstrap` only copies — it never installs from rubygems.org.** It populates the module's `vendor/bundle` from the Regent-shipped cache; if a gem is missing from that cache, that's a packaging bug, not a user-fixable one.
- **Errors point at `regent bootstrap`.** When the embedded runner can't find rspec or another required gem, the user is told to run `regent bootstrap` — never to install gems on the host.

Anything that requires host Ruby is a regression. If a new feature appears to need it, find an Artichoke-compatible solution or ship the gem in the bundled cache.

## Architecture Diagram

```
┌─────────────────────────────────────┐
│       Regent CLI (Rust)             │
│  cargo build --release              │
└────────────────┬────────────────────┘
                 │
    ┌────────────┴────────────┐
    │                         │
    ▼                         ▼
┌─────────────┐         ┌──────────────────┐
│ Rust Modules│         │ Artichoke Runtime│
│ - Generator │         │ - Ruby VM        │
│ - Validator │         │ - Gem Support    │
│ - Builder   │         │ - FFI Bridge     │
│ - Tester    │         │ - Stdlib         │
└─────────────┘         └──────────────────┘
    │                         │
    └────────────────┬────────┘
                     │
            ┌────────▼────────┐
            │ Unified Runtime │
            │ (Single Binary) │
            └─────────────────┘
```

## Key Files

### Rust FFI Bridge
- [src/ruby_interop.rs](../src/ruby_interop.rs) - Artichoke interoperability layer
- [src/artichoke_runtime.rs](../src/artichoke_runtime.rs) - Runtime configuration

### Ruby Bindings
- [lib/regent.rb](./regent.rb) - Main Ruby module
- [lib/regent/](./regent/) - Ruby submodules

## Using Gems in Regent

Regent can load and use any Ruby gem compatible with Artichoke Ruby:

```bash
# CLI usage
regent new my-module --with-gem puppet
regent new my-module --with-gem facter

# From Ruby code
require 'regent'
Regent.load_gem('puppet')
Regent.load_gem('rspec')
```

## Warning: Native Dependencies

Any feature or gem that relies on native extensions must be rebuilt or replaced with a pure-Ruby alternative for Artichoke. Native-dependent gems are not compatible out of the box and will fail to load unless a compatible build is provided.

## Base Gems Bundling

Regent should bundle the base gems required for all test runs to avoid re-downloading them. This includes the core RSpec stack and any pure-Ruby dependencies needed for Artichoke execution.

## Building for Distribution

### Create standalone binary

```bash
cargo build --release
# Binary: target/release/regent
```

### Include with gem

```bash
# The gem will include the compiled binary
gem build regent.gemspec
```

### Docker container

```dockerfile
FROM scratch
COPY target/release/regent /regent
ENTRYPOINT ["/regent"]
```

## Performance Considerations

### Rust Code Path (Fast)
- Module generation: ~50-100ms
- Validation: ~20-50ms
- File operations: Native speed

### Ruby Code Path (via Artichoke, Still Fast)
- Custom scripts: Optimized through JIT
- Gem code: Full Ruby compatibility
- Complex logic: Rust for performance-critical parts

### Optimization Tips

1. Use Rust for I/O-heavy operations
2. Use Artichoke Ruby for test execution and complex logic
3. Compile to release mode for deployment: `cargo build --release`
4. Use `--lto` for additional optimization

## Testing Artichoke Integration

```bash
# Run Rust tests
cargo test

# Test Ruby interop
cargo run -- new test_module
```

## Troubleshooting

### Issue: "Artichoke not found"
Ensure you have Rust 1.70+ and ran `cargo build`

### Issue: "Gem not loading"
Check Gemfile and ensure gem is compatible with Artichoke Ruby

### Issue: "Performance is slow"
Build with release profile: `cargo build --release`

## Additional Resources

- [Artichoke Ruby Docs](https://www.artichokeruby.org/)
- [Rust FFI Guide](https://docs.rust-embedded.org/book/interoperability/c-with-rust.html)
- [Regent Architecture](../ARCHITECTURE.md)

## Contributing to Artichoke Integration

To add new Artichoke features:

1. Update [src/ruby_interop.rs](../src/ruby_interop.rs)
2. Add tests in `src/tests/artichoke_tests.rs`
3. Update this documentation
4. Open a PR

---

**Note**: This is a hybrid project. Always consider whether to implement features in Rust (for performance) or Ruby (for flexibility).
