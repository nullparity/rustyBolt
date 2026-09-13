[English](README.md) | [Español](README.es.md)

# rustyBolt

A free, portable, cross-platform launcher for RuneLite and HDOS. It logs you in to your Jagex account and launches your client.

## Why did I make this?

The original Bolt is great, but it is a little difficult to use on macOS. The goal here is to remain OS agnostic, securely handle Jagex account information, and separate the concerns.

I play OSRS on a MacBook Pro, so there is also a **Wi-Fi mode** that reduces Wi-Fi network latency.

## Prerequisites

- One client from the approved list, installed and started once: [RuneLite](https://oldschool.runescape.wiki/w/RuneLite) or [HDOS](https://oldschool.runescape.wiki/w/HDOS). The Java that the RuneLite installer ships is enough.
- Java 11 or newer.
- A system keychain, to save a Jagex login. macOS and Windows have one. On Linux you need a Secret Service such as GNOME Keyring, KDE Wallet or KeePassXC; without one the launcher still runs, but cannot keep you signed in. If GNOME asks for a keyring password that your login password does not unlock, the login keyring was made with an older password: change it in Passwords and Keys.

## Install

Download from [Releases](https://github.com/nullparity/rustyBolt/releases).

| system | file | steps |
| --- | --- | --- |
| macOS | `.dmg` | Open. Drag `rustyBolt.app` to `Applications`. |
| Windows | `.msi` | Double-click. |
| Debian, Ubuntu | `.deb` | `sudo apt install ./rustybolt_*.deb` |
| Fedora, openSUSE | `.rpm` | `sudo dnf install ./rustybolt_*.rpm` (or `zypper`) |
| Other Linux | `.AppImage` | `chmod +x`, then double-click. Needs the distribution's `webkit2gtk-4.1` and GTK 3 (Fedora: `sudo dnf install webkit2gtk4.1`; for a tray icon also `libayatana-appindicator-gtk3`). |

## Use

1. Open rustyBolt.
2. **Add Jagex Account** and log in.
3. Pick a character, pick a client, click **PLAY**.

The language switch is in the header. Settings, Wi-Fi mode and JVM tuning live under **Settings**. The command line is in [docs/cli.md](docs/cli.md).

## Docs

- [Troubleshooting](docs/troubleshooting.md)
- [Command line](docs/cli.md)
- [How login works](docs/login.md)
- [Build from source and contributing](docs/building.md)
- [Architecture](docs/architecture.md)

## Disclaimer

rustyBolt is an unofficial project. It is not affiliated with Jagex, RuneLite or HDOS. Those parties are not responsible for any problem with rustyBolt or any damage that rustyBolt causes.

rustyBolt is not a game client. It runs the unmodified clients that the user installed. It cannot modify or automate gameplay. The launcher uses only the public login endpoints, and it never reads or alters game data.

RuneScape, Old School RuneScape and Jagex are trademarks of Jagex Limited.

## License

MIT. See [LICENSE](LICENSE).

The version pill in the top left glows when a newer release is out. Click it twice to update in place; a launcher installed from a package opens the download page instead. A GitHub token in **Settings** is optional: it raises the API rate limit and reaches a private repository. Note that the token sits in `launcher.json` in plain text.

Release assets carry Sigstore build provenance. To check a download:

```
gh attestation verify rustybolt_<version>_<platform>.dmg --repo nullparity/rustyBolt
```
