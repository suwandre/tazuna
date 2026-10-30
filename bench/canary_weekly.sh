#!/bin/zsh
# The weekly canary: canary.py on the routes tazuna uses, each on a key of its own (account log per key), at the effort tazuna
# uses them. SubRouter only (no Claude plan usage). Results in .model-check/canary.jsonl, read by scoreboard.py.
#   bench/canary_weekly.sh install   # launchd, Sundays 10:00 (rerun install after changing this file)
#   bench/canary_weekly.sh           # one run now
export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"
HERE="${0:A:h}"
if [ "$1" = install ]; then
  P="$HOME/Library/LaunchAgents/com.tazuna.canary.plist"
  cat > "$P" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>com.tazuna.canary</string>
  <key>ProgramArguments</key><array><string>/bin/zsh</string><string>$HERE/canary_weekly.sh</string></array>
  <key>StartCalendarInterval</key><dict><key>Weekday</key><integer>0</integer><key>Hour</key><integer>10</integer><key>Minute</key><integer>0</integer></dict>
  <key>StandardOutPath</key><string>$HERE/.model-check/canary-weekly.log</string>
  <key>StandardErrorPath</key><string>$HERE/.model-check/canary-weekly.log</string>
</dict></plist>
PLIST
  launchctl unload "$P" 2>/dev/null; launchctl load "$P" && echo "installed: Sundays 10:00, log in $HERE/.model-check/canary-weekly.log"
  exit 0
fi
cd "$HERE" || exit 1
source ~/.subrouter-key
echo "$(date '+%Y-%m-%d %H:%M') canary run"
python3 -I canary.py run deepseek-v4.1-flash-1017 --key-env SUBROUTER_CANARY_DS_KEY --effort xhigh
python3 -I canary.py run gpt-6.1-sol --key-env SUBROUTER_CANARY_SOL_KEY --effort xhigh
python3 -I scoreboard.py > /dev/null
