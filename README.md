# rustyBolt

A free, portable, cross-platform launcher for RuneLite and HDOS. It logs you in to your Jagex account and launches your client.

## Why did I make this?

Bolt is great, but it is a little difficult to use on macOS. The goal here is to remain OS agnostic, securely handle Jagex account information, and separate the concerns.

I play OSRS on a MacBook Pro, so there is also a **Wi-Fi mode** that reduces Wi-Fi network latency.

## Prerequisites

- One client from the approved list, installed and started once: [RuneLite](https://oldschool.runescape.wiki/w/RuneLite) or [HDOS](https://oldschool.runescape.wiki/w/HDOS).
- Java 11 or newer.
- A system keychain. macOS and Windows have one. On Linux you need GNOME Keyring, KDE Wallet or KeePassXC; rustyBolt refuses to run without one.

## Install

Download from [Releases](https://github.com/nullparity/rustyBolt/releases).

| system | file | steps |
| --- | --- | --- |
| macOS | `.tar.gz` | Unzip. Drag `rustyBolt.app` to `/Applications`. |
| Windows | `.msi` | Double-click. |
| Debian, Ubuntu | `.deb` | `sudo apt install ./rustybolt_*.deb` |
| Other Linux | `.AppImage` | `chmod +x`, then double-click. |

## Use

1. Open rustyBolt.
2. **Add Jagex Account** and log in.
3. Pick a character, pick a client, click **PLAY**.

Settings, Wi-Fi mode and JVM tuning live under **Settings**. The command line is in [docs/cli.md](docs/cli.md).

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
