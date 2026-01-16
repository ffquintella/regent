# Artichoke Ruby Integration Guide

## Overview

Regent is now integrated with Artichoke Ruby, a Ruby VM implementation written in Rust. This provides several advantages:

1. **Performance**: Ruby code runs at near-native speed through Rust compilation
2. **Compatibility**: Full support for Ruby language features and gems
3. **Interoperability**: Seamless calling between Rust and Ruby code
4. **Distribution**: Single standalone binary with no external Ruby dependency

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
