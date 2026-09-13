# Troubleshooting

**RuneLite is not installed.** Install it from the [wiki page](https://oldschool.runescape.wiki/w/RuneLite) and start it once. The error lists every place that the launcher looked. A jar in another place: set it under **Settings**, or run `rustybolt launch runelite --jar <path>`.

**HDOS is not installed.** Install it from the [wiki page](https://oldschool.runescape.wiki/w/HDOS) and start it once.

**No Java runtime of version 11 or newer with a desktop toolkit exists.** Install a runtime, or set a custom Java path under **Settings**. The launcher looks in `JAVA_HOME`, on `PATH`, in the standard places of each system and in the runtime that the RuneLite installer ships. On Linux a `-headless` package (Fedora's default `java-21-openjdk-headless`) has no toolkit and cannot open a window; the Java list marks it `headless`.

**The client starts with the wrong flags.** Check the command line with `rustybolt verify`, or adjust JVM settings under **Settings**. A runtime older than 24 drops the compact object headers, the string deduplication, the native access flag and the AOT cache; older than 21 drops `ZGenerational`; older than 15 uses G1 instead of ZGC.

**The desktop session died when the client started.** The machine ran out of memory. The launcher caps the heap at half the physical memory, but the client, the browser view and the desktop still need about 4 GB in total.

**Keychain unavailable.** rustyBolt stores the session in the system keychain only, so without one it runs but cannot keep you signed in. On Linux, install and start GNOME Keyring, KDE Wallet or KeePassXC (with its Secret Service integration on). `Object does not exist at path .../collection/login` means the login keyring was never created: log out and back in once. A keyring prompt that your login password does not unlock means the keyring was made with an older password; change it in Passwords and Keys.

**No tray icon on Linux.** The tray needs `libayatana-appindicator3`. Fedora does not install it by default: `sudo dnf install libayatana-appindicator-gtk3`. The launcher runs without it and prints one line.

**The AppImage does not start.** It uses the system's GTK 3 and WebKitGTK 4.1. Fedora: `sudo dnf install webkit2gtk4.1`. Debian and Ubuntu: `sudo apt install libwebkit2gtk-4.1-0`.

**Windows: the login page says the OAuth client does not exist.** Fixed in 0.3.1. Older versions cut the login URL at the first `&`.

**The session expired.** Log in again. The launcher keeps one session per Jagex account.

**The login dialog asks for my password.** See [login.md](login.md). This is the one-time port 80 setup on macOS and Linux.
