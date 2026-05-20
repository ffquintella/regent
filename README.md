# Regent - Puppet Development Kit in Rust

A high-performance, modern implementation of Puppet Development Kit (PDK) features in Rust with Ruby interoperability.

## ⚙️ Runtime Principle: Embedded Ruby Only

**Regent uses its embedded Artichoke Ruby runtime — implemented in Rust — for all Ruby execution. It does NOT depend on a host `ruby`, `gem`, or `bundle` install.**

- All required gems (rspec and friends) ship with Regent in a bundled gem cache and are installed into a **per-user bundle at `~/.regent/bundle`** by `regent bootstrap`. The embedded runner reads from there for every module — no per-module copies.
- Regent must never shell out to a host Bundler or Rubygems for normal operation.
- If a required gem is missing at runtime, the user is told to run `regent bootstrap` — never to `gem install` or `bundle install` on the host.

See [docs/ARTICHOKE_INTEGRATION.md](docs/ARTICHOKE_INTEGRATION.md) for details.

## 📚 Documentation

All project documentation has been organized in the [`docs/`](docs/) folder. Start with:

- **[00_START_HERE.md](docs/00_START_HERE.md)** - Quick orientation guide
- **[README.md](docs/README.md)** - Full project overview
- **[ROADMAP_PDK_FEATURES.md](docs/ROADMAP_PDK_FEATURES.md)** - Feature roadmap and implementation status
- **[QUICKSTART.md](docs/QUICKSTART.md)** - Getting started guide

### Key Documentation Files

| Document | Purpose |
|----------|---------|
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | System design and architecture overview |
| [IMPLEMENTATION_COMPLETE.md](docs/IMPLEMENTATION_COMPLETE.md) | Phase 1 & 2 completion status |
| [CONTRIBUTING.md](docs/CONTRIBUTING.md) | Contribution guidelines |
| [BUILD_PHASE_IMPLEMENTATION.md](docs/BUILD_PHASE_IMPLEMENTATION.md) | Build system detailed implementation |
| [ARTICHOKE_INTEGRATION.md](docs/ARTICHOKE_INTEGRATION.md) | Ruby/Rust integration details |

## 🚀 Quick Start

```bash
# Build the project
cargo build

# Run tests
cargo test

# Run the CLI
./target/debug/regent --help
```

## ✅ Current Status

- **Phase 1 (BUILD)**: ✅ 100% Complete - 34/34 tests passing
- **Phase 2 (TEST)**: ✅ 100% Complete - 80/80 tests passing
  - Week 1: Unit Test Framework ✅
  - Week 2: Multi-Version Testing Matrix ✅
  - Week 3: Test Fixtures Management ✅
  - Week 4: Integration Testing ✅
- **Total**: 114/114 tests passing

## 📦 Project Structure

```
regent/
├── src/
│   ├── builder/          # Phase 1: Build functionality
│   ├── tester/           # Phase 2: Test functionality
│   └── validator/        # Phase 3: Validation (planned)
├── spec/                 # Ruby tests
├── docs/                 # 📁 Documentation (see above)
├── Cargo.toml            # Rust dependencies
└── README.md             # This file
```

## 🔗 More Information

For detailed implementation information, roadmap, and feature status, please see the [documentation folder](docs/).

---

**Version**: 1.0  
**Last Updated**: January 16, 2026  
**Status**: Active Development
