# Build from source

You need a Rust toolchain of version 1.82 or newer from [rustup](https://rustup.rs). On Linux you also need the WebKitGTK 4.1, GTK 3, Ayatana AppIndicator and D-Bus development packages.

The dashboard stylesheet is compiled from `crates/rustybolt-cli/ui/app.css` by the Tailwind standalone CLI at build time; no Node.js. Install the pinned binary once:

```sh
scripts/install-tailwindcss.sh
```

It goes to `~/.local/bin`. Put that on `PATH`, or set `TAILWINDCSS` to the binary.

```sh
cargo test --workspace
cargo run -p rustybolt-cli -- help
cargo run -p rustybolt-cli -- configure
```

## Make a release

A tag that starts with `v` starts the release workflow. [dist](https://github.com/astral-sh/cargo-dist) generates `.github/workflows/release.yml` from `dist-workspace.toml`; edit the config, then run `dist generate` and commit both. The workflow builds `rustybolt` on a native runner for each target, makes one archive per target (`rustybolt-cli-<triple>.tar.gz`, `.zip` on Windows) with a `.sha256` file, and publishes the release with `sha256.sum`. `.github/workflows/packages.yml` runs alongside it and adds the installer packages:

| system | packages | built by |
| --- | --- | --- |
| macOS | `.dmg` with `rustyBolt.app` | `macos/build-app.sh`, `macos/build-dmg.sh` (hdiutil) |
| Windows | `.msi` | WiX 3 from `windows/rustybolt.wxs` |
| Linux | `.deb`, `.rpm`, `.AppImage` | `linux/build-deb.sh`, `linux/build-rpm.sh` (rpmbuild, soname requires), `linux/build-appimage.sh` (appimagetool) |

The archives feed the in-app update: the version pill in the dashboard glows when a newer release exists, and a click swaps the binary in place. A launcher installed from a `.deb`, `.rpm`, `.msi` or run as an AppImage opens the release page instead, so the package manager stays the owner of the file.

The Linux packages bundle no libraries. GTK and WebKitGTK come from the host: WebKitGTK starts its helper processes from a path compiled into the library, so a bundled copy only runs on a host with the build machine's layout. The Linux runners are Ubuntu 22.04, so the binaries run on any glibc 2.35 or newer (Debian 12, Ubuntu 22.04, Fedora 42 and later). Every release asset carries a Sigstore provenance attestation; `gh attestation verify <file> --repo nullparity/rustyBolt` checks one.

```sh
git tag v0.1.0
git push origin v0.1.0
```

The targets are macOS (arm64, amd64), Linux (arm64, amd64) and Windows (amd64, arm64). The Windows binary links the MSVC runtime statically, so it needs no redistributable. Pull requests and pushes to `main` run `cargo fmt`, `cargo clippy` and `cargo test` on the three systems.

## Test on virtual machines

A development machine hides most platform bugs: it has a system Java, RuneLite set up, and the login redirect installed. Clean VMs found every one of the fixes in 0.3.x, so run a release there before you trust it. Give the VM 4 GB or more; RuneLite's default heap is 2 GB. Each VM needs the RuneLite installer (its bundled runtime is the only Java on a clean machine) and, on Linux, a Secret Service with an unlocked login keyring.

## Contributing

Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` before you push. Continuous integration runs the same three commands on macOS, Linux and Windows.

Write comments and commit messages that state the code, not the edit.

Do not add an attribution line for an AI tool to a commit or a pull request. `AI-USAGE.md` states where this project uses AI.
