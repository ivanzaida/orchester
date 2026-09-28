#!/usr/bin/env bash
# Builds Orchester.app on a Mac: the release desktop and runner, the icon and Info.plist, signed
# ad hoc. The app is not notarized (see docs/stable-build.md, "macOS").
#   tools/package-macos.sh [--universal] [--with-mcp] [--build-dir <dir>] [--out-dir <dir>] [--no-sign]
#
#   --universal   Apple silicon and Intel in one app (needs both rustup targets; see below).
#   --with-mcp    Include the embedded MCP server, so agents can drive the app.
#   --build-dir   Cargo target directory (default: target/ of this checkout).
#   --out-dir     Where Orchester.app is written (default: <build-dir>/macos).
#   --no-sign     Leave the bundle unsigned (the linker still signs each program ad hoc on Apple
#                 silicon, but the bundle's Info.plist and icon are not sealed).
#
# Bundle layout (the desktop finds the runner beside itself, in Contents/MacOS):
#   Orchester.app/Contents/Info.plist
#   Orchester.app/Contents/MacOS/orchester
#   Orchester.app/Contents/MacOS/orchester-runner
#   Orchester.app/Contents/Resources/orchester.icns
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
build_dir="$repo/target"
out_dir=""
universal=0
with_mcp=0
sign=1
bundle_id="dev.orchester.desktop"
programs="orchester orchester-runner"

fail() {
  echo "package-macos: $*" >&2
  exit 1
}

while [ $# -gt 0 ]; do
  case "$1" in
    --universal) universal=1 ;;
    --with-mcp) with_mcp=1 ;;
    --no-sign) sign=0 ;;
    --build-dir) [ $# -ge 2 ] || fail "--build-dir needs a directory"; build_dir="$2"; shift ;;
    --out-dir) [ $# -ge 2 ] || fail "--out-dir needs a directory"; out_dir="$2"; shift ;;
    -h | --help) sed -n '2,17p' "$0"; exit 0 ;;
    *) fail "unknown option $1 (see --help)" ;;
  esac
  shift
done
[ "$(uname -s)" = "Darwin" ] || fail "this builds a macOS app bundle; run it on a Mac"
[ -n "$out_dir" ] || out_dir="$build_dir/macos"

version=$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\(.*\)"$/\1/p' "$repo/Cargo.toml")
[ -n "$version" ] || fail "cannot read the version from [workspace.package] in Cargo.toml"

# cargo build for the host, or for `$1` when given.
build() {
  local features=""
  [ "$with_mcp" = 1 ] && features="--features orchester-desktop/mcp"
  # shellcheck disable=SC2086 # $features is empty or two words on purpose.
  (cd "$repo" && cargo build --release --locked --target-dir "$build_dir" \
    -p orchester-desktop -p orchester-runner $features ${1:+--target "$1"})
}

staging="$out_dir/.Orchester.app.partial"
app="$out_dir/Orchester.app"
rm -rf "$staging"
mkdir -p "$staging/Contents/MacOS" "$staging/Contents/Resources"

if [ "$universal" = 1 ]; then
  targets="aarch64-apple-darwin x86_64-apple-darwin"
  for target in $targets; do
    rustup target list --installed | grep -x "$target" >/dev/null ||
      fail "the $target target is missing; install it with: rustup target add $target"
    build "$target"
  done
  for program in $programs; do
    lipo -create -output "$staging/Contents/MacOS/$program" \
      "$build_dir/aarch64-apple-darwin/release/$program" "$build_dir/x86_64-apple-darwin/release/$program"
  done
else
  build
  for program in $programs; do
    cp "$build_dir/release/$program" "$staging/Contents/MacOS/$program"
  done
fi

cp "$repo/crates/desktop/assets/icon/orchester.icns" "$staging/Contents/Resources/orchester.icns"
cat > "$staging/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>Orchester</string>
  <key>CFBundleExecutable</key>
  <string>orchester</string>
  <key>CFBundleIconFile</key>
  <string>orchester</string>
  <key>CFBundleIdentifier</key>
  <string>$bundle_id</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>Orchester</string>
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
plutil -lint -s "$staging/Contents/Info.plist" || fail "the generated Info.plist is invalid"

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
