#!/usr/bin/env bash
# Installs a release build of Orchester.app for daily use on a Mac, apart from the dev build in
# target/debug that agents rebuild. It builds the app from this checkout into its own target
# directory, packages it (tools/package-macos.sh) and moves it into ~/Applications. It keeps its
# database in ~/Library/Application Support/Orchester and its log in ~/Library/Logs/Orchester, so the
# installed app has its own project list. See docs/stable-build.md.
# Builds before the program's rename installed the app as Orchester App.app, beside the deprecated
# app's Orchester.app, which this replaces. An Orchester App.app left in the install folder is from an
# older build: the script never deletes it, refuses while it runs, and says so.
#   tools/install-macos.sh [--install-dir <dir>] [--build-dir <dir>] [--universal] [--import-dev-data]
#
#   --install-dir      Where the app goes (default: ~/Applications).
#   --build-dir        Cargo target directory (default: ~/Library/Application Support/Orchester/build),
#                      outside the repository so agent builds never touch it.
#   --universal        Apple silicon and Intel in one app.
#   --import-dev-data  Copy the dev build's application database (target/debug/orchester.db) to the
#                      installed app's. Refused when that one exists: it is never overwritten.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
install_dir="$HOME/Applications"
data_dir="$HOME/Library/Application Support/Orchester"
build_dir="$data_dir/build"
dev_dir="$repo/target/debug"
import_dev_data=0
package_options=""
# SQLite runs in WAL mode: after an unclean end, committed changes may still be in the -wal file.
database_files="orchester.db orchester.db-wal"

fail() {
  echo "install-macos: $*" >&2
  exit 1
}

while [ $# -gt 0 ]; do
  case "$1" in
    --install-dir) [ $# -ge 2 ] || fail "--install-dir needs a directory"; install_dir="$2"; shift ;;
    --build-dir) [ $# -ge 2 ] || fail "--build-dir needs a directory"; build_dir="$2"; shift ;;
    --universal) package_options="$package_options $1" ;;
    --import-dev-data) import_dev_data=1 ;;
    -h | --help) sed -n '2,17p' "$0"; exit 0 ;;
    *) fail "unknown option $1 (see --help)" ;;
  esac
  shift
done
[ "$(uname -s)" = "Darwin" ] || fail "this installs a macOS app; run it on a Mac"
# The bundle tools/package-macos.sh builds.
app="Orchester.app"
# The app's bundle as builds before the program's rename named it; it shares the data folder.
former_app="Orchester App.app"

# Refuses while a program of `$1` (a folder) is running. Nothing is ever stopped here.
assert_not_running() {
  local running
  running=$(ps -axo pid=,comm= | awk -v a="$1/orchester" -v b="$1/orchester-app" -v c="$1/orchester-runner" \
    '{ pid = $1; sub(/^ *[0-9]+ +/, ""); if ($0 == a || $0 == b || $0 == c) print pid }' | tr '\n' ' ')
  [ -z "$running" ] || fail "$2 is running from $1 (process $running). Quit it and run this script again."
}

assert_can_import_dev_data() {
  [ -f "$dev_dir/orchester.db" ] || fail "there is no dev database to import at $dev_dir/orchester.db"
  for file in $database_files; do
    [ ! -e "$data_dir/$file" ] ||
      fail "$data_dir/$file already exists; --import-dev-data never overwrites the installed app's data"
  done
  assert_not_running "$dev_dir" "The dev build of Orchester"
}

check() {
  assert_not_running "$install_dir/$app/Contents/MacOS" "The installed Orchester"
  assert_not_running "$install_dir/$former_app/Contents/MacOS" "The installed ${former_app%.app} of an older build"
  [ "$import_dev_data" = 0 ] || assert_can_import_dev_data
}

check
# shellcheck disable=SC2086 # $package_options holds whole options on purpose.
"$repo/tools/package-macos.sh" --build-dir "$build_dir" --out-dir "$build_dir/package" $package_options
commit=$(git -C "$repo" rev-parse --short HEAD)
[ -z "$(git -C "$repo" status --porcelain)" ] || commit="$commit with uncommitted changes"

# The build takes minutes; the app may have been started meanwhile.
check

# Replace the whole bundle by moving it into place: macOS may kill a program whose file changed
# in place, because it caches the code signature. It is copied beside the old one first.
mkdir -p "$install_dir"
had_database=0
[ ! -f "$data_dir/orchester.db" ] || had_database=1
rm -rf "$install_dir/.$app.new" "$install_dir/.$app.old"
ditto "$build_dir/package/$app" "$install_dir/.$app.new"
if ! { [ ! -e "$install_dir/$app" ] || mv "$install_dir/$app" "$install_dir/.$app.old"; } ||
  ! mv "$install_dir/.$app.new" "$install_dir/$app"; then
  fail "cannot replace $install_dir/$app; nothing was replaced (the old one may be .$app.old)"
fi
rm -rf "$install_dir/.$app.old"

if [ "$import_dev_data" = 1 ]; then
  mkdir -p "$data_dir"
  for file in $database_files; do
    # -n: never replace a file that appeared since the check.
    [ ! -f "$dev_dir/$file" ] || cp -n "$dev_dir/$file" "$data_dir/$file"
  done
  echo "Imported the dev database from $dev_dir"
fi

echo "Installed Orchester ($commit) to $install_dir/$app"
echo "Start it from Finder or Spotlight, or with: open \"$install_dir/$app\""
if [ -e "$install_dir/$former_app" ]; then
  echo "$install_dir/$former_app is the app from an older build (before its program was renamed). Do not start" \
    "it: it would open the data in $data_dir with an older build. Delete it yourself; this script never deletes it." >&2
fi
if [ "$had_database" = 1 ]; then
  echo "Before the first start of this build, with the app closed, copy aside $data_dir/orchester.db" \
    "(and orchester.db-wal) and each project's .orchester/orchester.db (and its -wal): this build may" \
    "migrate them, and an older build then refuses them. See docs/stable-build.md." >&2
fi
