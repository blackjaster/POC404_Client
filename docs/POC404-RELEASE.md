# Publishing POC404 (github.com/blackjaster/POC404_Client)

The launcher downloads the game from this repository's **releases**: `POC404-windows-x64.zip`, with
`SHA256SUMS` and `SHA256SUMS.sig` beside it. It installs nothing that wasn't signed by the key in
`crates/horizon-release-sign/release-keys.txt`.

## 1. The signing secret (once)
Repository > Settings > Secrets and variables > Actions > New repository secret:
name `RELEASE_SIGNING_KEY`, value: the one line in `RELEASE_SIGNING_KEY.txt`. Then delete that file.
Whoever holds this key can publish builds your launcher will trust.

## 2. The exporters (once, on your own computer)
The game prepares its files from *your* copy of Minecraft, and does it with a small bundle of
compiled classes, `exporters/`, that can only be built where Minecraft is installed. Needs: Minecraft
26.3 played once from the official launcher, a JDK (the version `research-lock.json` pins), Rust.
```
cargo build --release -p horizon-prepare
target\release\horizon-prepare --prepare
target\release\horizon-prepare compile-exporters
```
It prints a folder: copy its `server`, `client` and `manifest.json` into `exporters\` in this
repository, then commit and push. Only POC404's own classes are in it, no Minecraft files
(`python tools/check_prepare_exporters.py --binary target/release/horizon-prepare` checks that).

## 3. Each release
Actions > **POC404 release** > Run workflow, tag `v1.0.0` (a v and a number). About 30-60 minutes
(the first run). The result is a release with the three files above, and the launcher's Versions
page lists it.
