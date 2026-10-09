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
