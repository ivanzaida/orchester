# Replaces Orchester's bundle with a verified update once the app exited (crates/workspace/src/app_updates/install/bundle).
# The app starts it as: sh -c <this> orchester-update <pid> <bundle> <new bundle> <unpacked folder> <result file> <log>
# The renames stay on the bundle's volume, so each is whole or not done. On any failure the old bundle is put back and
# opened again, and why goes to the result file, which the next start reads.
pid="$1" app="$2" new="$3" unpacked="$4" result="$5" log="$6"
aside="$(dirname "$app")/.$(basename "$app").old"
exec >>"$log" 2>&1

say() {
  echo "$(date '+%Y-%m-%d %H:%M:%S') $*"
}

# Writes why to the result file and ends; the bundle is left as it is.
stop() {
  say "$1"
  printf '%s\n' "$1" >"$result"
  rm -rf "$unpacked"
  exit 1
}

# Puts the old bundle back, opens it again and stops with why.
restore() {
  if [ ! -e "$app" ] && [ -e "$aside" ]; then
    mv "$aside" "$app" || say "cannot put the old app back from $aside"
  fi
  /usr/bin/open "$app" || say "cannot open the old app again"
  stop "$1"
}

say "waiting for Orchester (process $pid) to exit"
waited=0
while kill -0 "$pid" 2>/dev/null; do
  # At most 10 minutes, in steps of 0.2 seconds.
  [ "$waited" -lt 3000 ] || stop "Orchester did not exit within 10 minutes; nothing was replaced"
  sleep 0.2
  waited=$((waited + 1))
done

rm -rf "$aside"
[ ! -e "$aside" ] || restore "cannot remove $aside, left by an earlier update"
mv "$app" "$aside" || restore "cannot move $app aside"
mv "$new" "$app" || restore "cannot move the new app into $app"
if ! /usr/bin/open "$app"; then
  mv "$app" "$new" || say "cannot move the new app out of $app"
  restore "the new app does not open"
fi
rm -rf "$aside" "$unpacked" || say "cannot remove the old app at $aside"
say "replaced $app"
