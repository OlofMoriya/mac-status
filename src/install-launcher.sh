#!/bin/bash
set -euo pipefail
plan=${1:?Missing plan path}
binary=${2:?Missing executable path}
[[ -r $plan && -x $binary ]] || exit 1
app="$HOME/Applications/Mac Status.app"
if [[ -e $app && ! -f $app/Contents/Resources/mac-status-plan ]]; then
  echo "Not replacing existing $app" >&2
  exit 1
fi
mkdir -p "$app/Contents/"{MacOS,Resources}
printf '%s\n' "$binary" > "$app/Contents/Resources/mac-status-binary"
printf '%s\n' "$plan" > "$app/Contents/Resources/mac-status-plan"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>dev.mac-status.launcher</string>
<key>CFBundleName</key><string>Mac Status</string>
<key>CFBundleIconFile</key><string>AppIcon.icns</string>
<key>CFBundleExecutable</key><string>launch</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
</dict></plist>
PLIST
cat > "$app/Contents/MacOS/launch" <<'LAUNCH'
#!/bin/bash
export PATH="/opt/homebrew/bin:/usr/local/bin:$HOME/.dotnet/tools:$HOME/go/bin:$HOME/.pi/agent/bin:/usr/bin:/bin:/usr/sbin:/sbin"
IFS= read -r plan < "$(dirname "$0")/../Resources/mac-status-plan"
IFS= read -r binary < "$(dirname "$0")/../Resources/mac-status-binary"
if [[ -r $plan && -x $binary ]]; then
  exec "$binary" --config "$plan" --gui
fi
/usr/bin/osascript -e 'display alert "Mac Status could not start" message "Check the Homebrew install and rerun mac-status --config PLAN.toml --install-launcher."'
LAUNCH
chmod +x "$app/Contents/MacOS/launch"
/bin/cat > "$app/Contents/Resources/AppIcon.icns"
/usr/bin/touch "$app"
/usr/bin/plutil -lint "$app/Contents/Info.plist"
printf 'Launcher installed: %s\n' "$app"
