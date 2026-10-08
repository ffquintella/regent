# Regent offline gem cache

`cache.tar.gz` is the committed prebuilt payload embedded in the Regent binary.
It includes the RSpec/Puppet/facts gems and their runtime dependency closure,
upstream license files, and original specification metadata. `cache.lock.json`
pins each upstream `.gem` archive's version and SHA-256, runtime requirements,
entrypoint, and the complete cache archive's SHA-256. Native extension gems are
excluded. The `ruby/2.6.0` directory is a cache layout identifier, not a host
Ruby requirement.

Normal builds and installations require no network access for gems:

```sh
python3 scripts/prepare-gem-cache.py          # verify and extract locally
python3 scripts/prepare-gem-cache.py --check  # verify the committed archive
python3 scripts/prepare-gem-cache.py --stage target/dist/bundled_gems
```

The extracted `ruby/` and `manifest.json` are generated and ignored. Packages
stage only this verified payload as non-executable data, preventing upstream
helper scripts from creating host-interpreter package dependencies.
`regent bootstrap` copies a complete sidecar
cache when available or extracts the embedded payload into the per-user bundle.
Removing the build checkout does not prevent a binary-only install from working.
An existing complete bundle is reused; an incomplete bundle is repaired.
`REGENT_BUNDLED_GEMS` can select another complete cache.

Maintainers can intentionally regenerate the archive with:

```sh
python3 scripts/prepare-gem-cache.py --rebuild
```

Only this explicit maintainer operation downloads from RubyGems. It verifies
the locked upstream hashes before extracting any code; it never runs Ruby,
RubyGems, or Bundler. Update the lock deliberately when changing versions,
review the upstream licenses, then rebuild and commit both lock and archive.
The manifest records hashes for every payload file; archive timestamps and
ownership are normalized for reproducible output. This packaging repair does
not expand the embedded engine's supported Ruby/Puppet APIs.
