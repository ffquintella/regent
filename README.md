# Regent - Puppet Development Kit in Rust

A high-performance, modern implementation of Puppet Development Kit (PDK) features in Rust with Ruby interoperability.

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
