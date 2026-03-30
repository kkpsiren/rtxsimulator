# Release Workflow

This repository does not currently have automated releases. Use the workflow below for every release so the workspace version, git tags, and published artifacts stay aligned.

## Versioning policy

- Bump the shared workspace version in `Cargo.toml`.
- Use one git tag per release in the form `vX.Y.Z`.
- Never create a tag without first updating the workspace version to the same number.
- If a tag already exists, do not reuse it. Cut the next version instead.

For example, because `v0.1.1` already exists in this repository, the next valid release after `0.1.0` is `0.1.2`.

## Preflight

1. Confirm the working tree only contains intended release changes.
2. Check existing tags:

```bash
git tag --list --sort=-version:refname | head
```

3. Verify the workspace version matches the intended next tag:

```bash
rg -n '^version = ' Cargo.toml crates/*/Cargo.toml
```

## Update version

1. Bump `[workspace.package].version` in the root `Cargo.toml`.
2. Update any workspace path dependency versions that are published externally. In this repo, the root workspace dependency on `rtxsimulator` must carry the same version so `rtxsimulator-cli` packages correctly.
3. Regenerate `Cargo.lock` so the workspace package entries pick up the new version.

## Verify before tagging

Run the full verification pass from a clean checkout:

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo audit
```

If the release includes dependency or packaging changes, also dry-run the packages you intend to publish:

```bash
cargo package -p rtxsimulator --allow-dirty
cargo package -p rtxsimulator-cli --allow-dirty
```

For the npm/WASM package, build the distributable package first:

```bash
wasm-pack build crates/wasm --target bundler --release
```

## Tag and push

After verification passes:

```bash
git add -A
git commit -m "Release vX.Y.Z"
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin main
git push origin vX.Y.Z
```

## Publish artifacts

Rust crates:

```bash
cargo publish -p rtxsimulator
cargo publish -p rtxsimulator-cli
```

WASM / npm package:

```bash
wasm-pack publish crates/wasm
```

## Post-release check

Confirm all three agree:

- `Cargo.toml` version
- latest git tag
- package version shown by the published artifact registry

If any of those disagree, fix the process with a new version instead of mutating an existing tag.
