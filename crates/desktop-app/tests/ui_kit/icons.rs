//! The icon list stays sorted by the design's icon names, with one glyph per icon.

use std::collections::BTreeSet;

use orchester_app::ui::icon::Icon;

/// The design's name of `icon`: `FolderGit2` is `folder-git-2`.
fn design_name(icon: Icon) -> String {
  let mut name = String::new();
  let mut previous: Option<char> = None;
  for letter in format!("{icon:?}").chars() {
    let starts_word =
      letter.is_ascii_uppercase() || (letter.is_ascii_digit() && !previous.is_some_and(|p| p.is_ascii_digit()));
    if starts_word && previous.is_some() {
      name.push('-');
    }
    name.push(letter.to_ascii_lowercase());
    previous = Some(letter);
  }
  name
}

#[test]
fn icons_are_sorted_by_their_design_names_with_one_glyph_each() {
  assert_eq!(design_name(Icon::FolderGit2), "folder-git-2");
  let names: Vec<String> = Icon::ALL.iter().copied().map(design_name).collect();
  let mut sorted = names.clone();
  sorted.sort();
  assert_eq!(names, sorted, "ui/icon.rs lists icons alphabetically");
  let glyphs: BTreeSet<char> = Icon::ALL.iter().map(|icon| icon.glyph()).collect();
  assert_eq!(glyphs.len(), Icon::ALL.len(), "no two icons share a glyph");
  assert!(
    Icon::ALL
      .iter()
      .all(|icon| ('\u{e000}'..='\u{f8ff}').contains(&icon.glyph())),
    "every glyph is in the font's private use area"
  );
}

fn word(bytes: &[u8], offset: usize) -> usize {
  u16::from_be_bytes(bytes[offset..offset + 2].try_into().expect("font word")) as usize
}
fn table(font: &[u8], name: &[u8; 4]) -> usize {
  (0..word(font, 4))
    .map(|index| 12 + index * 16)
    .find(|offset| &font[*offset..*offset + 4] == name)
    .map(|offset| u32::from_be_bytes(font[offset + 8..offset + 12].try_into().expect("table offset")) as usize)
    .expect("font table")
}
fn glyph_name(font: &[u8], glyph: usize) -> &str {
  let post = table(font, b"post");
  let count = word(font, post + 32);
  let wanted = word(font, post + 34 + glyph * 2);
  if wanted < 258 {
    return "";
  }
  let mut cursor = post + 34 + count * 2;
  for _ in 258..wanted {
    cursor += 1 + usize::from(font[cursor]);
  }
  let size = usize::from(font[cursor]);
  std::str::from_utf8(&font[cursor + 1..cursor + 1 + size]).expect("glyph name")
}
fn font_glyph(font: &[u8], codepoint: usize) -> usize {
  let cmap = table(font, b"cmap");
  let subtable = (0..word(font, cmap + 2))
    .map(|index| cmap + 4 + index * 8)
    .map(|offset| cmap + u32::from_be_bytes(font[offset + 4..offset + 8].try_into().expect("cmap offset")) as usize)
    .find(|offset| word(font, *offset) == 4)
    .expect("format 4 cmap");
  let count = word(font, subtable + 6) / 2;
  let end = subtable + 14;
  let start = end + count * 2 + 2;
  let delta = start + count * 2;
  let range = delta + count * 2;
  for index in 0..count {
    if codepoint >= word(font, start + index * 2) && codepoint <= word(font, end + index * 2) {
      let shift = word(font, delta + index * 2);
      let range_offset = range + index * 2;
      let distance = word(font, range_offset);
      let glyph = if distance == 0 {
        codepoint
      } else {
        word(
          font,
          range_offset + distance + (codepoint - word(font, start + index * 2)) * 2,
        )
      };
      return (glyph + shift) % 65536;
    }
  }
  0
}
#[test]
fn instructions_icons_match_bundled_font_names() {
  let font = include_bytes!("../../assets/fonts/lucide.ttf");
  for (icon, name) in [
    (Icon::Braces, "braces"),
    (Icon::CalendarClock, "calendar-clock"),
    (Icon::Replace, "replace"),
    (Icon::ArchiveRestore, "archive-restore"),
    (Icon::ChevronsUp, "chevrons-up"),
    (Icon::Inbox, "inbox"),
    (Icon::PackageOpen, "package-open"),
    (Icon::PinOff, "pin-off"),
    (Icon::RotateCw, "rotate-cw"),
    (Icon::Signpost, "signpost"),
  ] {
    assert_eq!(glyph_name(font, font_glyph(font, icon.glyph() as usize)), name);
  }
}

/// The glyphs of the reasons a tool request came to the person (04c `vZcoN`); the font names `shield-question`
/// `shield-question-mark` and `shield-x` `shield-close`.
#[test]
fn icons_of_approval_reasons_match_bundled_font_names() {
  let font = include_bytes!("../../assets/fonts/lucide.ttf");
  for (icon, name) in [
    (Icon::FolderLock, "folder-lock"),
    (Icon::ShieldQuestion, "shield-question-mark"),
    (Icon::ShieldX, "shield-close"),
    (Icon::Upload, "upload"),
    (Icon::GitBranch, "git-branch"),
  ] {
    assert_eq!(glyph_name(font, font_glyph(font, icon.glyph() as usize)), name);
  }
}
