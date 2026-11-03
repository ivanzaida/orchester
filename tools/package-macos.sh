#!/usr/bin/env bash
# Builds Orchester.app on a Mac from a release build: the app (`orchester-app`, the program
# `orchester`) with the runner, the icon and Info.plist. It keeps its data in ~/Library. Signed ad
# hoc unless a Developer ID identity is given; notarized and stapled when the notary's credentials
# are given too (see docs/stable-build.md, "macOS").
#   tools/package-macos.sh [--universal] [--build-dir <dir>] [--out-dir <dir>] [--no-sign]
#       [--identity <name>] [--notary-key <file> --notary-key-id <id> --notary-issuer <id>] [--archives]
#
#   --universal      Apple silicon and Intel in one app (needs both rustup targets; see below).
#   --build-dir      Cargo target directory (default: target/ of this checkout).
#   --out-dir        Where the bundle is written (default: <build-dir>/macos).
#   --no-sign        Leave the bundle unsigned (the linker still signs each program ad hoc on Apple
#                    silicon, but the bundle's Info.plist and icon are not sealed).
#   --identity       The Developer ID identity to sign with, as `security find-identity` names it, in a
#                    keychain on the search list: hardened runtime and a secure timestamp. Or the
#                    environment's ORCHESTER_MACOS_IDENTITY.
#   --notary-key     The App Store Connect API key's .p8 file, to notarize and staple; needs the identity
#                    and the two below. Or the environment's ORCHESTER_NOTARY_KEY.
#   --notary-key-id  That key's id. Or the environment's ORCHESTER_NOTARY_KEY_ID.
#   --notary-issuer  Its issuer id. Or the environment's ORCHESTER_NOTARY_ISSUER.
#   --archives       Also pack the release's two files beside the bundle, Apple silicon only:
#                    Orchester-<version>-macos-aarch64.dmg (the app and an /Applications link) and
#                    Orchester-<version>-macos-aarch64.zip (the app, for the updater).
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
archives=0
identity="${ORCHESTER_MACOS_IDENTITY:-}"
notary_key="${ORCHESTER_NOTARY_KEY:-}"
notary_key_id="${ORCHESTER_NOTARY_KEY_ID:-}"
notary_issuer="${ORCHESTER_NOTARY_ISSUER:-}"
programs="orchester orchester-runner"

fail() {
  echo "package-macos: $*" >&2
  exit 1
}

while [ $# -gt 0 ]; do
  case "$1" in
    --universal) universal=1 ;;
    --no-sign) sign=0 ;;
    --archives) archives=1 ;;
    --build-dir) [ $# -ge 2 ] || fail "--build-dir needs a directory"; build_dir="$2"; shift ;;
    --out-dir) [ $# -ge 2 ] || fail "--out-dir needs a directory"; out_dir="$2"; shift ;;
    --identity) [ $# -ge 2 ] || fail "--identity needs a name"; identity="$2"; shift ;;
    --notary-key) [ $# -ge 2 ] || fail "--notary-key needs a file"; notary_key="$2"; shift ;;
    --notary-key-id) [ $# -ge 2 ] || fail "--notary-key-id needs an id"; notary_key_id="$2"; shift ;;
    --notary-issuer) [ $# -ge 2 ] || fail "--notary-issuer needs an id"; notary_issuer="$2"; shift ;;
    -h | --help) sed -n '2,31p' "$0"; exit 0 ;;
    *) fail "unknown option $1 (see --help)" ;;
  esac
  shift
done
[ "$(uname -s)" = "Darwin" ] || fail "this builds a macOS app bundle; run it on a Mac"
[ -n "$out_dir" ] || out_dir="$build_dir/macos"

# What was asked goes together, before the build's minutes are spent.
notarize=0
if [ -n "$notary_key$notary_key_id$notary_issuer" ]; then
  [ -n "$notary_key" ] && [ -n "$notary_key_id" ] && [ -n "$notary_issuer" ] ||
    fail "notarizing needs all of --notary-key, --notary-key-id and --notary-issuer"
  [ -f "$notary_key" ] || fail "there is no notary key file at $notary_key"
  [ -n "$identity" ] || fail "notarizing needs a Developer ID identity (--identity); Apple refuses an ad hoc signature"
  notarize=1
fi
if [ "$sign" = 0 ]; then
  [ -z "$identity" ] || fail "--no-sign and a Developer ID identity exclude each other"
  [ "$archives" = 0 ] || fail "--archives packs a signed app; it does not go with --no-sign"
fi
if [ "$archives" = 1 ]; then
  # The cloud refuses a second file under one name, so a universal or Intel build needs names of its own
  # (docs/stable-build.md, "Releases from CI").
  [ "$universal" = 0 ] || fail "--archives names its files for Apple silicon; it does not go with --universal"
  [ "$(uname -m)" = "arm64" ] || fail "--archives names its files for Apple silicon; this Mac builds for $(uname -m)"
fi
[ -z "$identity" ] || security find-identity -v -p codesigning | grep -F "\"$identity\"" >/dev/null ||
  fail "no valid signing identity \"$identity\" in the keychains on the search list"

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

  # The runner is signed first, then the bundle seals it with the rest.
  if [ -n "$identity" ]; then
    codesign --force --options runtime --timestamp --sign "$identity" "$staging/Contents/MacOS/orchester-runner"
    codesign --force --options runtime --timestamp --sign "$identity" "$staging"
    codesign --verify --strict "$staging"
  elif [ "$sign" = 1 ]; then
    # Ad hoc ("-"): no Apple identity, so the app runs only on Macs that built or trust it, and it
    # is not notarized.
    codesign --force --sign - --timestamp=none "$staging/Contents/MacOS/orchester-runner"
    codesign --force --sign - --timestamp=none "$staging"
    codesign --verify --strict "$staging"
  fi

  rm -rf "$app"
  mv "$staging" "$app"
  echo "Built $app (version $version$([ "$universal" = 1 ] && echo ", universal"))"
}

# Runs the command after the first argument up to that many times, 20 seconds apart: a ticket reaches Apple's servers
# some time after the submission is accepted, and hdiutil sometimes finds a folder busy that nothing holds.
again() {
  local tries="$1" try=1
  shift
  until "$@"; do
    [ "$try" -lt "$tries" ] || return 1
    echo "package-macos: $1 failed; trying again ($try of $tries)" >&2
    try=$((try + 1))
    sleep 20
  done
}

# Notarizes the file $1 (a zip or a disk image) and waits for the verdict; a verdict other than Accepted prints the
# submission's log and stops the script. notarytool takes the key's id and issuer only as arguments; nothing here echoes
# a command, so neither reaches the output.
submit() {
  local answer id status
  echo "Notarizing $(basename "$1")"
  # It exits non-zero on a rejection too; the verdict below decides, with the log.
  answer=$(xcrun notarytool submit "$1" --key "$notary_key" --key-id "$notary_key_id" --issuer "$notary_issuer" \
    --wait --timeout 30m --output-format json) || true
  id=$(plutil -extract id raw -o - - <<<"$answer" 2>/dev/null) || id=""
  status=$(plutil -extract status raw -o - - <<<"$answer" 2>/dev/null) || status=""
  [ -n "$id" ] || fail "notarytool gave no submission for $(basename "$1"): $answer"
  if [ "$status" != "Accepted" ]; then
    echo "package-macos: notarization of $(basename "$1") ended as \"$status\" (submission $id); its log:" >&2
    xcrun notarytool log "$id" --key "$notary_key" --key-id "$notary_key_id" --issuer "$notary_issuer" >&2 ||
      echo "package-macos: notarytool could not fetch the log" >&2
    fail "Apple did not notarize $(basename "$1")"
  fi
  echo "Apple accepted $(basename "$1") (submission $id)"
}

staple() {
  again 5 xcrun stapler staple "$1" || fail "cannot staple the ticket to $(basename "$1")"
  xcrun stapler validate "$1" || fail "the ticket stapled to $(basename "$1") does not validate"
}

# Stops the script unless Gatekeeper's verdict on $1, asked with the rest of the arguments, is a notarized Developer ID.
assert_gatekeeper_accepts() {
  local file="$1" verdict
  shift
  verdict=$(spctl -a -vv "$@" "$file" 2>&1) || fail "Gatekeeper refuses $(basename "$file"): $verdict"
  case "$verdict" in
    *"source=Notarized Developer ID"*) echo "$verdict" ;;
    *) fail "Gatekeeper does not take $(basename "$file") as notarized: $verdict" ;;
  esac
}

# Notarizes the bundle $1 and staples its ticket. notarytool takes no folder, so a zip of it goes up, and is dropped.
notarize_app() {
  local app="$1" upload="$out_dir/.$(basename "$1").notary.zip"
  ditto -c -k --keepParent "$app" "$upload"
  submit "$upload"
  rm -f "$upload"
  staple "$app"
  assert_gatekeeper_accepts "$app" -t exec
}

# The release's two files from the bundle $1, which is stapled by now, so both work offline at their first start. The
# disk image is signed and notarized as a file of its own: Gatekeeper asks about it as it is opened.
pack_archives() {
  local app="$1" base="Orchester-$version-macos-aarch64"
  local zip="$out_dir/$base.zip" dmg="$out_dir/$base.dmg" staging="$out_dir/.$base.partial"
  rm -f "$zip" "$dmg"
  ditto -c -k --keepParent "$app" "$zip"
  rm -rf "$staging"
  mkdir -p "$staging"
  ditto "$app" "$staging/$(basename "$app")"
  ln -s /Applications "$staging/Applications"
  again 5 hdiutil create -quiet -volname "Orchester" -srcfolder "$staging" -format UDZO -ov "$dmg" ||
    fail "hdiutil cannot make $(basename "$dmg")"
  rm -rf "$staging"
  if [ -n "$identity" ]; then
    codesign --force --timestamp --sign "$identity" "$dmg"
    codesign --verify --strict "$dmg"
  fi
  if [ "$notarize" = 1 ]; then
    submit "$dmg"
    staple "$dmg"
    assert_gatekeeper_accepts "$dmg" -t open --context context:primary-signature
  fi
  echo "Packed $zip"
  echo "Packed $dmg"
}

bundle "Orchester" orchester dev.orchester.app
rm -rf "$programs_dir"
[ "$notarize" = 0 ] || notarize_app "$out_dir/Orchester.app"
[ "$archives" = 0 ] || pack_archives "$out_dir/Orchester.app"
