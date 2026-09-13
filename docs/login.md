# How login works

**Add Jagex Account** in the desktop window opens the Jagex login in a small in-app browser view. When the `jagex:` URL scheme points to rustyBolt, the login opens in your default browser instead, so your password manager works. The macOS app bundle registers the scheme; on Linux, `linux/install.sh` registers the desktop entry as the handler.

Jagex sends the last redirect of the login to `http://localhost` on port 80. Only the app that owns port 80 can receive it. Windows lets the launcher listen there. macOS and Linux reserve the port, so the first browser login shows a one-time setup dialog that asks for your password through the system prompt:

- **macOS** installs a pf rule that forwards loopback port 80 to the launcher, plus a LaunchDaemon that turns the rule on only while a login is in progress. Port 80 stays free the rest of the time. The same setup runs from the terminal with `sudo macos/install-login-redirect.sh` (add `remove` to uninstall).
- **Linux** grants the launcher binary `cap_net_bind_service` with `setcap`, so it can listen on port 80 during a login. An update of the binary removes the capability; the dialog then returns once.

If another program already uses port 80, the launcher says so and logs in inside its own window instead.

Set `RUSTYBOLT_LOGIN=window` or `RUSTYBOLT_LOGIN=browser` to force one mode.

## The local API

The dashboard talks to the launcher over a loopback HTTP server on a random port. The server refuses any request whose `Host` is not that address, any browser request from another origin, and any `/api/` request without the per-launch token that only the served page and the port file (`launcher.port` in the runtime directory, mode 0600) hold. A web page in your browser therefore cannot read your accounts, launch the game or sign you out.

## Session storage

The launcher keeps one session per Jagex account in the system keychain: the macOS Keychain, the Windows Credential Manager, or the Secret Service on Linux (GNOME Keyring, KDE Wallet, KeePassXC). One entry named `rustybolt` / `sessions` holds every session. No token is ever printed, and no session sits in a plain file.

Without a keychain the launcher runs, shows a warning, and cannot save a login; a login whose session cannot be saved reports that. On Linux, start a Secret Service provider first. A session file from a version before the keychain moves into the keychain on the first start, and the file goes away.

Config, cache and other data:

| system | config and data | cache and runtime |
| --- | --- | --- |
| Linux | `$XDG_CONFIG_HOME/rustybolt`, `$XDG_DATA_HOME/rustybolt` | `$XDG_CACHE_HOME/rustybolt`, `$XDG_RUNTIME_DIR/rustybolt` |
| macOS | `~/Library/Application Support/rustybolt` | `~/Library/Caches/rustybolt` |
| Windows | `%APPDATA%\rustybolt` | `%LOCALAPPDATA%\rustybolt` |
