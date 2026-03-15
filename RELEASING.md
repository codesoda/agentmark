# Releasing AgentMark

## Prerequisites

- Push access to the repository
- All CI checks passing on `main`

## Steps

### 1. Update version numbers

Bump the version in all three locations — they must stay in sync:

- `packages/cli/Cargo.toml` → `version = "X.Y.Z"`
- `packages/extension/manifest.json` → `"version": "X.Y.Z"`
- `packages/extension/package.json` → `"version": "X.Y.Z"`

Then update `Cargo.lock`:

```sh
cargo check
```

### 2. Commit the version bump

```sh
git add packages/cli/Cargo.toml packages/extension/manifest.json packages/extension/package.json Cargo.lock
git commit -m "chore: bump version to X.Y.Z"
git push origin main
```

### 3. Tag and push

```sh
git tag vX.Y.Z
git push origin vX.Y.Z
```

This triggers the release workflow which:
- Cross-compiles the CLI for macOS (arm64 + x86_64) and Linux (x86_64 + aarch64)
- Builds the Chrome extension and embeds it in each binary
- Creates a **draft** GitHub Release with all artifacts and SHA-256 checksums

### 4. Publish the release

1. Go to https://github.com/codesoda/agentmark/releases
2. Review the draft release and auto-generated notes
3. Edit the description if needed
4. Click **Publish release**

## Release artifacts

Each release includes:

| Artifact | Description |
|---|---|
| `agentmark-vX.Y.Z-x86_64-apple-darwin.tar.gz` | macOS Intel |
| `agentmark-vX.Y.Z-aarch64-apple-darwin.tar.gz` | macOS Apple Silicon |
| `agentmark-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` | Linux x86_64 |
| `agentmark-vX.Y.Z-aarch64-unknown-linux-gnu.tar.gz` | Linux ARM64 |
| `checksums-sha256.txt` | SHA-256 checksums for all archives |

Each archive contains a single `agentmark` binary with the Chrome extension embedded.

## Installing from a release

Download and extract the binary for your platform:

```sh
# macOS Apple Silicon
curl -sSL https://github.com/codesoda/agentmark/releases/latest/download/agentmark-vX.Y.Z-aarch64-apple-darwin.tar.gz | tar xz
./agentmark-vX.Y.Z-aarch64-apple-darwin/agentmark install-extension
```

Or use the installer script (which still builds from source):

```sh
curl -sSL https://raw.githubusercontent.com/codesoda/agentmark/main/install.sh | bash
```

## Version scheme

This project uses [semantic versioning](https://semver.org/). While in `0.x.y`, minor bumps may include breaking changes.
