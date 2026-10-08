# Regent - Puppet Development Kit in Rust

A high-performance, modern implementation of Puppet Development Kit (PDK) features in Rust with Ruby interoperability.

## ⚙️ Runtime Principle: Embedded Ruby Only

**Regent uses its embedded Artichoke Ruby runtime — implemented in Rust — for all Ruby execution. It does NOT depend on a host `ruby`, `gem`, or `bundle` install.**

- All required gems (rspec and friends) ship with Regent in a bundled gem cache and are installed into a **per-user bundle** by `regent bootstrap`. The embedded runner reads from there for every module — no per-module copies.
  - Unix / macOS: `~/.regent/bundle`
  - Windows: `%APPDATA%\Regent\bundle`
- Regent must never shell out to a host Bundler or Rubygems for normal operation.
- If a required gem is missing at runtime, the user is told to run `regent bootstrap` — never to `gem install` or `bundle install` on the host.

The prebuilt gem cache is embedded in the binary, so `cargo install --path . --locked`
and standalone binary copies can bootstrap offline even after the build checkout
is removed. `make install` and platform packages also stage a verified cache.
Run `regent bootstrap` once, then `regent test` in your module directory.
See [the cache maintenance guide](assets/bundled_gems/README.md) for pinned
versions, checksum verification, and package preparation.

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
| [PUBLISHING_MODULES.md](docs/PUBLISHING_MODULES.md) | Publishing modules to the Forge or another repository |

### Agent orchestration

Regent uses a **dual-orchestrator** architecture: Claude plans and reviews,
Codex/GPT implements. Four documents define it, and `regent new` scaffolds the
same four into every module it creates.

| Document | Purpose |
|----------|---------|
| [AGENTS.md](AGENTS.md) | Normative multi-agent contract: roles, routing, budgets |
| [CLAUDE.md](CLAUDE.md) | Claude as CTO / architect / reviewer |
| [skills/claude/SKILLS.md](skills/claude/SKILLS.md) | Claude strategic-orchestrator routing rules |
| [skills/codex/SKILLS.md](skills/codex/SKILLS.md) | Codex engineering-orchestrator routing rules |

The runtime decides the model family. In Claude Code every agent runs on a
Claude model: Sonnet by default, Opus on escalation, Fable for simulation. In
Codex every agent runs on a GPT model: GPT-6 Mini by default, then GPT-6, GPT
Terra for long context, and GPT Sol for simulation. Crossing families is a
handoff between runtimes, never a call to the other family's model.

`AGENTS.md` is the single source of truth; the other three defer to it.
`tests/orchestration_docs.rs` fails the build if they drift apart.

## 🚀 Quick Start

```bash
# Build the project
cargo build

# Run tests
cargo test

# Run the CLI
./target/debug/regent --help
```

## 📤 Publishing a Module

```bash
# Build and upload to the Puppet Forge
regent publish --token "$MY_FORGE_TOKEN"

# See what would be uploaded, without uploading
regent publish --dry-run

# Upload to any other repository (Artifactory, Nexus, an internal mirror, …)
regent publish --url 'https://repo.example.com/puppet/{name}/{version}/{filename}' --username ci-user
```

Credentials come from `--token`, `--token-file`, `$REGENT_FORGE_TOKEN`,
`$REGENT_PUBLISH_TOKEN`, `$PDK_FORGE_TOKEN`, or `~/.regent/forge_token`.
See [docs/PUBLISHING_MODULES.md](docs/PUBLISHING_MODULES.md) for the full flag
reference.

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
│   ├── validator/        # Phase 3: Validation (planned)
│   └── publisher/        # Forge / repository publishing
├── skills/
│   ├── claude/SKILLS.md  # Claude strategic-orchestrator routing
│   └── codex/SKILLS.md   # Codex engineering-orchestrator routing
├── templates/agents/     # Agent docs scaffolded by `regent new`
├── spec/                 # Ruby tests
├── docs/                 # 📁 Documentation (see above)
├── AGENTS.md             # Multi-agent orchestration specification
├── CLAUDE.md             # Claude-specific behaviour
├── Cargo.toml            # Rust dependencies
└── README.md             # This file
```

## 🔗 More Information

For detailed implementation information, roadmap, and feature status, please see the [documentation folder](docs/).

---

**Version**: 1.0  
**Last Updated**: January 16, 2026  
**Status**: Active Development
