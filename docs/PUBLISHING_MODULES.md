# Publishing Modules (`regent publish`)

`regent publish` uploads a built module package to the [Puppet Forge](https://forge.puppet.com)
or to any other HTTP-reachable module repository. Like the rest of Regent, it is
pure Rust: no host `ruby`, `gem`, `bundle` or `puppet` is involved, and no
external upload tool is shelled out to.

## Quick start

```bash
regent publish --token "$MY_FORGE_TOKEN"
```

That resolves the module in the current directory, builds
`pkg/<name>-<version>.tar.gz` if it isn't already there, checks that the version
isn't already on the Forge, and `POST`s the tarball to
`https://forgeapi.puppet.com/v3/releases`.

Preview a release without uploading anything:

```bash
regent publish --dry-run
```

## What it does, in order

1. Loads and validates `metadata.json` (name, semver version, author, license,
   dependency requirements).
2. Warns about empty `summary`, `source` and `project_page` — the Forge accepts
   a release without them, but the module page looks unfinished.
3. Locates the package: `--file` if given, otherwise
   `<module>/pkg/<name>-<version>.tar.gz`, building it when missing (pass
   `--no-build` to fail instead).
4. For Forge targets, asks `GET /v3/releases/<name>-<version>` whether the
   version already exists and refuses to overwrite it unless `--force` is
   passed. The check fails open: if the API is unreachable, the upload proceeds.
5. Uploads, and on failure surfaces the repository's own error message.

## Credentials

Regent never prompts for or stores a token. It looks, in order, at:

1. `--token <TOKEN>`
2. `--token-file <PATH>` (whitespace-trimmed file contents)
3. `$REGENT_FORGE_TOKEN`
4. `$REGENT_PUBLISH_TOKEN`
5. `$PDK_FORGE_TOKEN` (so existing PDK setups keep working)
6. `~/.regent/forge_token` — `%APPDATA%\Regent\forge_token` on Windows

The token is sent as `Authorization: Bearer <token>`. Forge API keys are created
under *My Profile → API Tokens* on forge.puppet.com.

For repositories that use HTTP Basic auth instead, pass `--username` together
with `--password` or `$REGENT_PUBLISH_PASSWORD`. Prefer the environment variable
or a token file over a flag, so the secret stays out of your shell history.

## Publishing somewhere other than the Forge

### A Forge-compatible mirror

Anything that implements the Forge v3 release endpoint works by pointing
`--forge-url` at it:

```bash
regent publish --forge-url https://forge.internal.example.com --token-file ~/.regent/forge_token
```

### A generic repository

`--url` switches to a plain upload: the tarball becomes the raw request body
(`Content-Type: application/gzip`), sent with `PUT` by default or `POST` via
`--method post`. The URL may contain `{name}`, `{version}` and `{filename}`
placeholders:

```bash
# Artifactory generic repo
regent publish \
  --url 'https://artifactory.example.com/artifactory/puppet-modules/{name}/{version}/{filename}' \
  --username ci-user

# Nexus raw repo, extra headers, POST
regent publish \
  --url 'https://nexus.example.com/repository/puppet/{filename}' \
  --method post \
  --header 'X-Api-Key: …'
```

`--header` may be repeated and accepts either `Name: value` or `Name=value`.

## Flags

| Flag | Purpose |
|------|---------|
| `--file <PATH>` | Publish this tarball instead of `pkg/<name>-<version>.tar.gz` |
| `--no-build` | Fail instead of building a missing package |
| `--forge-url <URL>` | Forge API base URL (default `https://forgeapi.puppet.com`) |
| `--url <URL>` | Publish to a generic repository URL instead of a Forge API |
| `--method <put\|post>` | HTTP method for `--url` (default `put`) |
| `--token <TOKEN>` / `--token-file <PATH>` | Bearer credentials |
| `--username <USER>` / `--password <PASS>` | HTTP Basic credentials |
| `--header <Name: value>` | Extra request header (repeatable) |
| `--force` | Upload even if the Forge already has this version |
| `--dry-run` | Report what would be uploaded, and stop |

## CI usage

```bash
regent validate
regent test
regent build
REGENT_FORGE_TOKEN="$FORGE_TOKEN" regent publish --no-build
```

`--no-build` keeps the publish step honest: it uploads the artifact the build
step produced, or fails.

## Implementation

- [src/publisher/mod.rs](../src/publisher/mod.rs) — targets, credential
  resolution, package lookup, publish orchestration
- [src/publisher/upload.rs](../src/publisher/upload.rs) — multipart assembly,
  HTTP transport, error-body formatting
- [src/cli/publish.rs](../src/cli/publish.rs) — CLI wiring
- [tests/publish.rs](../tests/publish.rs) — offline end-to-end coverage
