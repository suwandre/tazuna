#!/bin/zsh
# The daily model check.
#   bench/model_check_daily.sh install   # copy the check to ~/.local/share/tazuna/model-check and run it daily at 9:00
#                                        # (launchd; macOS keeps background jobs out of ~/Desktop, so the job runs a copy:
#                                        # rerun install after changing model_check.py or roles.json)
#   (no argument)                        # one run: report in last.txt next to this script, a notification on new flags
export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"
HERE="${0:A:h}"
DEST="$HOME/.local/share/tazuna/model-check"
if [ "$1" = install ]; then
  mkdir -p "$DEST"
  cp "$HERE/model_check.py" "$HERE/roles.json" "$HERE/model_check_daily.sh" "$DEST/"
  P="$HOME/Library/LaunchAgents/com.tazuna.model-check.plist"
  cat > "$P" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>com.tazuna.model-check</string>
  <key>ProgramArguments</key><array><string>/bin/zsh</string><string>$DEST/model_check_daily.sh</string></array>
  <key>StartCalendarInterval</key><dict><key>Hour</key><integer>9</integer><key>Minute</key><integer>0</integer></dict>
  <key>StandardErrorPath</key><string>$DEST/launchd.err</string>
</dict>
</plist>
PLIST
  launchctl bootout "gui/$(id -u)/com.tazuna.model-check" 2>/dev/null
  launchctl bootstrap "gui/$(id -u)" "$P" && echo "installed: daily at 9:00, report in $DEST/last.txt"
  exit
fi
cd "$HERE" || exit 1
export MODEL_CHECK_STATE="$HERE/state"
for f in ~/.aa-key ~/.ci-key ~/.el-key; do [ -f "$f" ] && source "$f"; done
out=$(python3 model_check.py 2>&1)
print -r -- "$out" > last.txt
n=$(print -r -- "$out" | grep -c '^- ')
if [ "$n" -gt 0 ]; then
  osascript -e "display notification \"$n new model flag(s): see ~/.local/share/tazuna/model-check/last.txt\" with title \"tazuna model check\""
fi
