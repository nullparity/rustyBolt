# Troubleshooting

**RuneLite is not installed.** Install it from the [wiki page](https://oldschool.runescape.wiki/w/RuneLite) and start it once. The error lists every place that the launcher looked. A jar in another place: set it under **Settings**, or run `rustybolt launch runelite --jar <path>`.

**HDOS is not installed.** Install it from the [wiki page](https://oldschool.runescape.wiki/w/HDOS) and start it once.

**No Java runtime of version 11 or newer exists.** Install a runtime, or set a custom Java path under **Settings**. The launcher looks in `JAVA_HOME`, on `PATH` and in the standard places of each system.

**The client starts with the wrong flags.** Check the command line with `rustybolt verify`, or adjust JVM settings under **Settings**. A runtime older than 24 drops the compact object headers, the string deduplication, the native access flag and the AOT cache.

**no keychain is available to hold the Jagex session.** rustyBolt stores the session in the system keychain only. On Linux, install and start GNOME Keyring, KDE Wallet or KeePassXC (with its Secret Service integration on), then start rustyBolt again.

**The session expired.** Log in again. The launcher keeps one session per Jagex account.

**The login dialog asks for my password.** See [login.md](login.md). This is the one-time port 80 setup on macOS and Linux.
