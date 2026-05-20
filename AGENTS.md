See [CLAUDE.md](CLAUDE.md) for full agent instructions.

## TL;DR

Regent ships with an **embedded Artichoke Ruby runtime written in Rust**. There is **no host Ruby dependency** — and there must never be one.

- Do not shell out to `ruby`, `gem`, `bundle`, `rspec`, or `rake`.
- All Ruby code runs through Artichoke via `RubyEnvironment`.
- Gem dependencies ship in a pre-built cache; `regent bootstrap` copies them into the module's `vendor/bundle`.
- Missing-gem errors instruct the user to run `regent bootstrap`, never to install on the host.

Full rationale and rules: [CLAUDE.md](CLAUDE.md) and [docs/ARTICHOKE_INTEGRATION.md](docs/ARTICHOKE_INTEGRATION.md).
