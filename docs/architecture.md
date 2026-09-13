# Architecture

## The split

```mermaid
graph TD
    A[rustybolt-auth: OAuth2 PKCE state machine, no input or output] --> C[rustybolt-core]
    B[rustybolt-jdk: Java discovery, JVM argv] --> C
    E[rustybolt-security: egress allowlist, CSP policy] --> C
    C[rustybolt-core: paths, config, sessions, client lookup, launch] --> D[rustybolt-cli: CLI & configuration dashboard]
    E --> D
```

| crate | owns | knows about |
| --- | --- | --- |
| `rustybolt-auth` | the Jagex OAuth2 PKCE flow | nothing, no input or output |
| `rustybolt-jdk` | Java discovery and JVM arguments | the file system only |
| `rustybolt-security` | egress allowlist and Content Security Policy | nothing, no input or output |
| `rustybolt-core` | paths, config, sessions, client lookup, launch | `rustybolt-auth`, `rustybolt-jdk`, `rustybolt-security`, HTTP |
| `rustybolt-cli` | portable CLI driver and configuration UI | `rustybolt-core`, `rustybolt-jdk`, `rustybolt-security` |

`CONTRACT.md` holds the public API of every crate.

### Why the split helps a port

In the original Bolt, `Browser::LoginWindow` is at the same time the window, the HTTP interceptor and the OAuth client. Java discovery and the process launch sit in two platform files of the launcher window class. A port to a new system touches all three concerns.

In rustyBolt a new shell implements the user interface only. It gives each URL that its browser view reaches to `LoginFlow::on_navigation`, and it runs the action that comes back. The protocol, the Java rules and the file layout do not change.

## What works today

- Log in with your Jagex account. The launcher keeps the session in the system keychain, so the next start needs no login. Without a keychain the launcher still runs; it cannot keep you signed in and says so.
- Pick a character. The launcher offers the one you opened last.
- Find RuneLite or HDOS where its own installer put it. You can also name the jar.
- Find Java on its own. The launcher looks in `JAVA_HOME`, on `PATH`, in the standard places of each system and in the runtime that the RuneLite installer ships. A Linux runtime without a desktop toolkit (a `-headless` package) is listed but never chosen. You can also name a Java binary.
- Start RuneLite with a tuned set of JVM flags. The launcher drops each flag that your Java version does not accept, and caps the heap at half the physical memory.
- Show the dashboard in English or Spanish, from the system locale or the header switch.
- Wi-Fi mode sends a UDP keepalive to the default gateway every 10ms to stop Wi-Fi sleep lag, and measures the round trip with a TCP connect every 5s for the latency readout.
- Closing the application window hides it to the system tray so the background keepalive stays active.
- Lock down network egress to only official Jagex endpoints and localhost. The local API refuses any request without the per-launch token, a wrong `Host`, or a foreign `Origin`, so a web page in your browser cannot drive the launcher.
- Start faster from the second run on, through a startup cache that the launcher rebuilds when the client updates.
- Show the client under its own name in the Dock, the process list and the GNOME top bar, not as `java`.
- Keep RuneLite settings in a home of its own, so the launcher never touches `~/.runelite`. One command copies your existing settings in.
- Force RuneLite profile settings, for example a fixed graphics block.
- Wrap the launch in your own command, for example a wrapper script or a sandbox.
- Read login values from a secret manager, such as the 1Password command line tool.
- Write a GC log per client, and remove the log when that client ends.
- Use the native desktop application or the command line tool. Both do the same work.

Out of scope for now: the official RS3 and OSRS native clients, the plugin library and the Lua overlay. Those parts of the original Bolt do not touch the three seams that this project divides.

## The tuned launch profile

The default RuneLite profile comes from `rl-launcher`. `rustybolt verify` shows the exact command line that a launch runs.

| setting | value | note |
| --- | --- | --- |
| heap | `-Xms2g -Xmx2g` | a fixed heap, so the collector never grows it; capped at half the physical memory, and `-Xms` goes when the cap bites |
| collector | `-XX:+UseZGC` | `-XX:+ZGenerational` on features 21 to 23; G1 below 15, where ZGC is experimental |
| stack | `-Xss2m` | |
| feature 24 and newer | compact object headers, string deduplication, native access | older JDKs reject them |
| AOT cache | `-XX:AOTCacheOutput=` then `-XX:AOTCache=` | the first run writes it, later runs read it |
| open packages | seven `--add-opens` values | the client needs them for reflection |
| macOS | Metal java2d, the system appearance, the Dock name and icon, `--hw-accel METAL` | macOS only; elsewhere the client picks its renderer |
| client | `--launch-mode REFLECT` | `REFLECT` keeps the client in this JVM |

`--launch-mode REFLECT` matters. In the fork mode the RuneLite launcher starts a second JVM with its own `-Xmx768m` and drops every flag above.

`rustybolt_jdk::Tuning::flags(feature)` is pure. The caller gives the feature number, so a machine with one JDK can still test every gate. `rustybolt_core::plan` builds the command that `rustybolt_core::launch` runs, so a dry run and a real launch never differ.

## Faults of the original Bolt that this project corrects

| original Bolt | rustyBolt |
| --- | --- |
| `std::rand` makes the state and the verifier | the operating system generator makes them |
| the `id_token` goes to standard output | no token is printed |
| the credential file keeps the default mode | the session lives in the system keychain |
| the launcher downloads the client itself | the launcher starts the client that the user installed |
| Java discovery reads `JAVA_HOME` and `PATH` | discovery also reads the standard locations |
