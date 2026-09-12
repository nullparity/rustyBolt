#!/bin/sh
# Lets rustyBolt receive the Jagex consent redirect (http://localhost) from
# the system browser. macOS reserves port 80, so a pf rule forwards loopback
# port 80 to rustyBolt's consent port. The rule is only loaded while the file
# /Users/Shared/rustyBolt/login-active exists: rustyBolt creates it when a
# login starts and removes it when the login ends, so port 80 stays free the
# rest of the time. A LaunchDaemon watches that directory and toggles the rule.
#
#   sudo macos/install-login-redirect.sh install
#   sudo macos/install-login-redirect.sh remove
set -eu

NAME="net.runelite.rustybolt"
# /etc/pf.conf only evaluates anchors under com.apple/, so nest there.
ANCHOR="com.apple/$NAME"
ANCHOR_FILE="/etc/pf.anchors/$NAME"
HELPER="/Library/Application Support/rustyBolt/login-redirect.sh"
DAEMON="/Library/LaunchDaemons/$NAME.pf.plist"
FLAG_DIR="/Users/Shared/rustyBolt"
PORT=21080

if [ "$(id -u)" -ne 0 ]; then
    echo "run with sudo" >&2
    exit 1
fi

if [ "${1:-install}" = "remove" ]; then
    launchctl bootout system "$DAEMON" 2>/dev/null || true
    pfctl -a "$ANCHOR" -F all 2>/dev/null || true
    rm -f "$DAEMON" "$ANCHOR_FILE" "$HELPER"
    rmdir "$(dirname "$HELPER")" 2>/dev/null || true
    rm -rf "$FLAG_DIR"
    echo "Removed the rustyBolt login redirect."
    exit 0
fi

cat > "$ANCHOR_FILE" <<RULES
rdr pass on lo0 inet proto tcp from any to 127.0.0.1 port 80 -> 127.0.0.1 port $PORT
rdr pass on lo0 inet6 proto tcp from any to ::1 port 80 -> ::1 port $PORT
RULES
chmod 644 "$ANCHOR_FILE"

# The flag directory belongs to the user who installs, so only that user can
# turn the forward on.
mkdir -p "$FLAG_DIR"
if [ -n "${SUDO_USER:-}" ]; then
    chown "$SUDO_USER" "$FLAG_DIR"
fi
chmod 755 "$FLAG_DIR"
rm -f "$FLAG_DIR/login-active"

mkdir -p "$(dirname "$HELPER")"
cat > "$HELPER" <<HELPER_SCRIPT
#!/bin/sh
# Loads the rustyBolt port 80 forward while a login is active, else clears it.
if [ -e "$FLAG_DIR/login-active" ]; then
    /sbin/pfctl -E 2>/dev/null
    /sbin/pfctl -a "$ANCHOR" -f "$ANCHOR_FILE" 2>/dev/null
else
    /sbin/pfctl -a "$ANCHOR" -F all 2>/dev/null
fi
HELPER_SCRIPT
chmod 755 "$HELPER"

cat > "$DAEMON" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>$NAME.pf</string>
    <key>ProgramArguments</key>
    <array>
        <string>/bin/sh</string>
        <string>$HELPER</string>
    </array>
    <key>WatchPaths</key>
    <array>
        <string>$FLAG_DIR</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>ThrottleInterval</key>
    <integer>1</integer>
</dict>
</plist>
PLIST
chmod 644 "$DAEMON"

# Clear anything a previous version left loaded.
pfctl -a "$ANCHOR" -F all 2>/dev/null || true
pfctl -a "$NAME" -F all 2>/dev/null || true
launchctl bootout system "$DAEMON" 2>/dev/null || true
launchctl bootstrap system "$DAEMON"

echo "Installed: localhost:80 -> localhost:$PORT during rustyBolt logins."
