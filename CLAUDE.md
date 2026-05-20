# Regent — Agent Instructions

These instructions apply to any AI agent (Claude Code, Copilot, Cursor, etc.) working in this repository. Human contributors should follow them too.

## ⚙️ Core Principle: Embedded Ruby Only — No Host Ruby

Regent is a self-contained Rust binary with an **embedded Ruby runner (Artichoke), implemented in Rust**. It must run on machines that have no `ruby`, `gem`, or `bundle` on PATH.

**Never write code, scripts, or docs that assume a host Ruby toolchain.**

### Hard rules

1. **No shelling out to host Ruby tooling.** Do not introduce `Command::new("ruby")`, `Command::new("gem")`, `Command::new("bundle")`, `Command::new("rspec")`, `Command::new("rake")`, or similar in any Rust code path that runs during normal user operation.
2. **All Ruby execution goes through Artichoke** via `RubyEnvironment` / `crate::ruby_interop`. If a feature needs Ruby, it must run inside the embedded interpreter.
3. **Gems live in a per-user bundle: `~/.regent/bundle`.** This is the single canonical location for installed gems; the embedded Ruby runner reads from it for every module. Required gems (rspec, rspec-core, rspec-expectations, rspec-support, …) are distributed as a pre-built gem cache discovered in this order:
   - `$REGENT_BUNDLED_GEMS`
   - `~/.regent/bundle`
   - `<exe_dir>/bundled_gems`, `<exe_dir>/../share/regent/bundled_gems`, `<exe_dir>/../bundled_gems`
   - Dev fallbacks: `assets/bundled_gems`, `vendor/bundle` in the repo
   See [src/tester/bundled_gems.rs](src/tester/bundled_gems.rs).
4. **`regent bootstrap` copies into `~/.regent/bundle`, never installs from rubygems.org.** It populates the per-user bundle from the Regent-shipped cache and writes `export REGENT_BUNDLED_GEMS=…` into the user's shell rc files. If a gem is missing from the shipped cache, that's a Regent packaging bug — fix the cache, do not ask the user to `gem install`.
5. **Missing-dependency errors point at `regent bootstrap`.** When the embedded runner can't find rspec or another required gem, surface `missing_dependency_hint(...)` from [src/cli/bootstrap.rs](src/cli/bootstrap.rs). Never tell the user to install a host Ruby, gem, or bundler.
6. **Test scripts and CI must work without host Ruby.** Any new tests, fixtures, or CI jobs that require Ruby must drive Artichoke through the regent binary — not call `bundle exec` / `rspec` directly.

### When in doubt

- A new feature seems to "need" `bundle install` → ship the gem in the bundled cache instead.
- A new feature seems to "need" `ruby -e ...` → eval the Ruby through `RubyEnvironment`.
- An Artichoke incompatibility blocks the feature → file it, write a workaround inside the embedded runner, or pre-process in Rust. Do **not** fall back to host Ruby silently.

Anything that re-introduces a host Ruby dependency is a regression and should be rejected in review.

## Other conventions

- Version lives in `Cargo.toml` and is mirrored in `vscode-extension/package.json`. Keep them in sync.
- Architecture details: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
- Ruby integration details: [docs/ARTICHOKE_INTEGRATION.md](docs/ARTICHOKE_INTEGRATION.md).
