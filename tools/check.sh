#!/usr/bin/env bash
# Repository quality gate without tests, the bash counterpart of tools/check.ps1 run without -Tests: formatting,
# clippy with warnings denied, the 600-line file limit and the UI literals. Run from anywhere: bash tools/check.sh
# It needs bash, cargo (with the nightly toolchain for rustfmt) and perl, which macOS and Git for Windows ship.
# The pipelines' Check step on macOS runs it; the tests of a task are run by its own step, by name.

set -u

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
max_lines=599
failed=0

cd "$repo" || exit 1

# rustfmt.toml uses nightly-only options; the pinned stable toolchain would ignore them.
echo "== rustfmt"
if metadata=$(cargo +nightly metadata --no-deps --format-version 1); then
  members=$(printf '%s' "$metadata" | perl -MJSON::PP -e '
    my $m = decode_json(join("", <STDIN>));
    my %member = map { $_ => 1 } @{ $m->{workspace_members} };
    print "$_->{name}\n" for grep { $member{ $_->{id} } } @{ $m->{packages} };')
  for package in $members; do
    echo "== rustfmt $package"
    cargo +nightly fmt -p "$package" -- --check || failed=1
  done
else
  failed=1
fi

echo "== clippy"
cargo clippy --workspace --all-targets --all-features -- -D warnings || failed=1

echo "== file size (max $max_lines lines)"
# Lines as check.ps1 counts them (ReadAllLines): a last line without a final newline counts too.
oversized=$(find crates -name '*.rs' -type f -not -path '*/target/*' -print0 |
  xargs -0 perl -e '
    my $max = shift;
    for my $file (@ARGV) {
      open(my $fh, "<", $file) or die "$file: $!";
      local $/;
      my $text = <$fh>;
      my $lines = () = $text =~ /\n/g;
      $lines++ if length($text) && substr($text, -1) ne "\n";
      print "$lines $file\n" if $lines > $max;
    }' "$max_lines" | sort -rn)
if [ -n "$oversized" ]; then
  echo "$oversized"
  failed=1
fi

# UI code styles through lurq theme roles and shows text through i18n keys. Design values live only in the app's
# src/theme/. See .claude/skills/orchester-ui. The rules are check.ps1's, case-insensitive as Select-String is.
echo "== ui literals (theme roles and i18n only)"
ui_root="crates/desktop-app/src"
violations=$(find "$ui_root" -name '*.rs' -type f -not -path "$ui_root/theme/*" -print0 |
  xargs -0 perl -ne '
    my @rules = (
      ["hex color literal", qr/"#[0-9A-Fa-f]{3,8}"/i],
      ["concrete color", qr/\bColor::(from_hex|rgb|rgba|new)\b/i],
      ["inline text style", qr/\bTextStyle\s*\{|\bText::styled\(|\bfont_family\b|\bfont_size\b/i],
      ["hardcoded user-facing text", qr/\b(Text|Button)::new\(\s*"|\.(placeholder|with_title|tooltip|label)\(\s*"/i],
      ["numeric spacing/radius/border", qr/\.(padding\w*|spacing|gap|rounded|corner_radius\w*|border\w*)\(\s*-?[0-9]/i],
    );
    for my $rule (@rules) {
      if ($_ =~ $rule->[1]) { (my $line = $_) =~ s/^\s+|\s+$//g; print "$rule->[0]  $ARGV:$.  $line\n"; }
    }
    close ARGV if eof;')
if [ -n "$violations" ]; then
  echo "$violations"
  failed=1
fi

if [ "$failed" -ne 0 ]; then
  echo "check: FAILED"
  exit 1
fi
echo "check: OK"
