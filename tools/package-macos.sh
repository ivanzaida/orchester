#!/usr/bin/env bash
# Builds Orchester.app on a Mac from a release build: the app (`orchester-app`, the program
# `orchester`) with the runner, the icon and Info.plist, signed ad hoc. It keeps its data in
# ~/Library. The app is not notarized (see docs/stable-build.md, "macOS").
#   tools/package-macos.sh [--universal] [--build-dir <dir>] [--out-dir <dir>] [--no-sign]
#
#   --universal   Apple silicon and Intel in one app (needs both rustup targets; see below).
#   --build-dir   Cargo target directory (default: target/ of this checkout).
#   --out-dir     Where the bundle is written (default: <build-dir>/macos).
#   --no-sign     Leave the bundle unsigned (the linker still signs each program ad hoc on Apple
#                 silicon, but the bundle's Info.plist and icon are not sealed).
#
# Bundle layout (the app finds the runner beside itself, in Contents/MacOS):
#   Orchester.app/Contents/Info.plist
#   Orchester.app/Contents/MacOS/orchester
#   Orchester.app/Contents/MacOS/orchester-runner
#   Orchester.app/Contents/Resources/orchester.icns
# Builds before the program's rename made `Orchester App.app` (the app, as `orchester-app`) and an
# `Orchester.app` of the deprecated app; this bundle keeps the app's identifier, dev.orchester.app.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
build_dir="$repo/target"
out_dir=""
universal=0
sign=1
programs="orchester orchester-runner"

fail() {
  echo "package-macos: $*" >&2
  exit 1
}

while [ $# -gt 0 ]; do
  case "$1" in
    --universal) universal=1 ;;
    --no-sign) sign=0 ;;
    --build-dir) [ $# -ge 2 ] || fail "--build-dir needs a directory"; build_dir="$2"; shift ;;
    --out-dir) [ $# -ge 2 ] || fail "--out-dir needs a directory"; out_dir="$2"; shift ;;
    -h | --help) sed -n '2,20p' "$0"; exit 0 ;;
    *) fail "unknown option $1 (see --help)" ;;
  esac
  shift
done
[ "$(uname -s)" = "Darwin" ] || fail "this builds a macOS app bundle; run it on a Mac"
[ -n "$out_dir" ] || out_dir="$build_dir/macos"

# The app's own version (docs/stable-build.md, "Releases from CI").
version=$(sed -n '/^\[package\]/,/^\[/s/^version = "\(.*\)"$/\1/p' "$repo/crates/desktop-app/Cargo.toml")
[ -n "$version" ] || fail "cannot read the version from [package] in crates/desktop-app/Cargo.toml"

# cargo build for the host, or for `$1` when given. Every build includes the app's MCP server.
# ORCHESTER_APP_INSTALLED_BUILD makes this orchester-app the installed app: the fixed MCP address and the token in the
# secret store (docs/stable-build.md). Only this script and tools/install.ps1 set it.
build() {
  (cd "$repo" && ORCHESTER_APP_INSTALLED_BUILD=1 cargo build --release --locked --target-dir "$build_dir" \
    -p orchester-app -p orchester-runner ${1:+--target "$1"})
}

# The built programs, one universal file each with --universal.
programs_dir="$out_dir/.programs"
rm -rf "$programs_dir"
mkdir -p "$programs_dir"
if [ "$universal" = 1 ]; then
  targets="aarch64-apple-darwin x86_64-apple-darwin"
  for target in $targets; do
    rustup target list --installed | grep -x "$target" >/dev/null ||
      fail "the $target target is missing; install it with: rustup target add $target"
    build "$target"
  done
  for program in $programs; do
    lipo -create -output "$programs_dir/$program" \
      "$build_dir/aarch64-apple-darwin/release/$program" "$build_dir/x86_64-apple-darwin/release/$program"
  done
else
  build
  for program in $programs; do
    cp "$build_dir/release/$program" "$programs_dir/$program"
  done
fi

# Info.plist of the app named $1 whose program is $2, with the bundle identifier $3.
info_plist() {
  cat <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>$1</string>
  <key>CFBundleExecutable</key>
  <string>$2</string>
  <key>CFBundleIconFile</key>
  <string>orchester</string>
  <key>CFBundleIdentifier</key>
  <string>$3</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>$1</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>$version</string>
  <key>CFBundleVersion</key>
  <string>$version</string>
  <key>LSApplicationCategoryType</key>
  <string>public.app-category.developer-tools</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
PLIST
}

# Writes $out_dir/<$1>.app for the program $2 with the runner beside it, bundle identifier $3.
bundle() {
  local name="$1" program="$2" bundle_id="$3"
  local staging="$out_dir/.$name.app.partial"
  local app="$out_dir/$name.app"
  rm -rf "$staging"
  mkdir -p "$staging/Contents/MacOS" "$staging/Contents/Resources"
  cp "$programs_dir/$program" "$programs_dir/orchester-runner" "$staging/Contents/MacOS/"
  # The icon's sources stay in crates/desktop/assets/icon (tools/app-icon.ps1), which is no longer built;
  # crates/desktop-app/assets/icon/orchester.ico is this icon too.
  cp "$repo/crates/desktop/assets/icon/orchester.icns" "$staging/Contents/Resources/orchester.icns"
  info_plist "$name" "$program" "$bundle_id" > "$staging/Contents/Info.plist"
  plutil -lint -s "$staging/Contents/Info.plist" || fail "the generated Info.plist of $name.app is invalid"

  if [ "$sign" = 1 ]; then
    # Ad hoc ("-"): no Apple identity, so the app runs only on Macs that built or trust it, and it
    # is not notarized. The runner is signed first, then the bundle seals it with the rest.
    codesign --force --sign - --timestamp=none "$staging/Contents/MacOS/orchester-runner"
    codesign --force --sign - --timestamp=none "$staging"
    codesign --verify --strict "$staging"
  fi

  rm -rf "$app"
  mv "$staging" "$app"
  echo "Built $app (version $version$([ "$universal" = 1 ] && echo ", universal"))"
}

bundle "Orchester" orchester dev.orchester.app
rm -rf "$programs_dir"
