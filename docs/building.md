# Build from source

You need a Rust toolchain of version 1.82 or newer from [rustup](https://rustup.rs). On Linux you also need the WebKitGTK, GTK, Ayatana AppIndicator and D-Bus development packages.

```sh
cargo test --workspace
cargo run -p rustybolt-cli -- help
cargo run -p rustybolt-cli -- configure
```

## Make a release

A tag that starts with `v` starts the release workflow. The workflow builds `rustybolt` on a native runner for each target, packages it, uploads the packages and `checksums.txt`, and publishes the release.

| system | packages | built by |
| --- | --- | --- |
| macOS | `.tar.gz` with `rustyBolt.app` | `macos/build-app.sh` |
| Windows | `.msi`, `.zip` | WiX 3 from `windows/rustybolt.wxs` |
| Linux | `.deb`, `.AppImage`, `.tar.gz` | `linux/build-deb.sh`, `linux/build-appimage.sh` (linuxdeploy) |

```sh
git tag v0.1.0
git push origin v0.1.0
```

The targets are macOS (arm64, amd64), Linux (arm64, amd64) and Windows (amd64, arm64). Pull requests and pushes to `main` run `cargo fmt`, `cargo clippy` and `cargo test` on the three systems.

## Contributing

Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` before you push. Continuous integration runs the same three commands on macOS, Linux and Windows.

Write comments and commit messages that state the code, not the edit.

Do not add an attribution line for an AI tool to a commit or a pull request. `AI-USAGE.md` states where this project uses AI.
