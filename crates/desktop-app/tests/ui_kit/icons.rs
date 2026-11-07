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
    (Icon::FileSearch, "file-search"),
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

/// The glyphs of the update's status item and question (`acFkz`); the font names `circle-arrow-up` `arrow-up-circle`
/// and `circle-check` `check-circle-2`.
#[test]
fn update_install_icons_match_bundled_font_names() {
  let font = include_bytes!("../../assets/fonts/lucide.ttf");
  for (icon, name) in [
    (Icon::CircleArrowUp, "arrow-up-circle"),
    (Icon::CircleCheck, "check-circle-2"),
    (Icon::Hourglass, "hourglass"),
    (Icon::CircleStop, "circle-stop"),
    (Icon::Pause, "pause"),
    (Icon::Database, "database"),
    (Icon::RotateCcw, "rotate-ccw"),
  ] {
    assert_eq!(glyph_name(font, font_glyph(font, icon.glyph() as usize)), name);
  }
}

#[test]
fn decision_connection_scale_matches_bundled_font_name() {
  let font = include_bytes!("../../assets/fonts/lucide.ttf");
  let codepoint = (0xe000..0xf000)
    .find(|codepoint| glyph_name(font, font_glyph(font, *codepoint)) == "scale")
    .expect("the bundled font includes scale");
  assert_eq!(
    Icon::Scale.glyph() as usize,
    codepoint,
    "scale codepoint: {codepoint:x}"
  );
}

/// The glyphs of *Compact* and the compaction's lines (ORC-309, `GENCC`, `dPq1I`, `l5YmWb`).
#[test]
fn compact_icons_match_bundled_font_names() {
  let font = include_bytes!("../../assets/fonts/lucide.ttf");
  for (icon, name) in [
    (Icon::FoldVertical, "fold-vertical"),
    (Icon::FoldHorizontal, "fold-horizontal"),
  ] {
    assert_eq!(glyph_name(font, font_glyph(font, icon.glyph() as usize)), name);
  }
}
/// The glyphs of the @ menu's rows and its empty state (`qhBhC`, `zzJ78`); the font names `square-check` by its
/// former name `check-square-2`.
#[test]
fn mention_menu_icons_match_bundled_font_names() {
  let font = include_bytes!("../../assets/fonts/lucide.ttf");
  for (icon, name) in [
    (Icon::SquareCheck, "check-square-2"),
    (Icon::Play, "play"),
    (Icon::SearchX, "search-x"),
  ] {
    assert_eq!(glyph_name(font, font_glyph(font, icon.glyph() as usize)), name);
  }
}

/// The glyphs of Edit and *Rewind to here* on an earlier message (ORC-330's `GYAPg`, `QJb0b`, `Zm7kq`).
#[test]
fn rewind_icons_match_bundled_font_names() {
  let font = include_bytes!("../../assets/fonts/lucide.ttf");
  for (icon, name) in [
    (Icon::MessageSquareX, "message-square-x"),
    (Icon::History, "history"),
    (Icon::Ellipsis, "ellipsis"),
    (Icon::ArrowDown, "arrow-down"),
    (Icon::ListChecks, "list-checks"),
  ] {
    assert_eq!(glyph_name(font, font_glyph(font, icon.glyph() as usize)), name);
  }
}

/// The glyph of *Fork from here* and the fork's line (ORC-330's `GYAPg`, `RT5T7`).
#[test]
fn fork_icons_match_bundled_font_names() {
  let font = include_bytes!("../../assets/fonts/lucide.ttf");
  assert_eq!(
    glyph_name(font, font_glyph(font, Icon::GitFork.glyph() as usize)),
    "git-fork"
  );
}

/// The glyphs of a run's pull request CI and of *Fix CI* (ORC-335, 10b `mxt1S`, 11c `NMHzc`).
#[test]
fn ci_icons_match_bundled_font_names() {
  let font = include_bytes!("../../assets/fonts/lucide.ttf");
  for (icon, name) in [
    (Icon::Repeat, "repeat"),
    (Icon::Wrench, "wrench"),
    (Icon::GitPullRequest, "git-pull-request"),
  ] {
    assert_eq!(glyph_name(font, font_glyph(font, icon.glyph() as usize)), name);
  }
}
/// The glyph of the Memory page's *Memory settings* (`YHyk4` in `S15` `BJQn0`).
#[test]
fn memory_settings_icon_matches_bundled_font_name() {
  let font = include_bytes!("../../assets/fonts/lucide.ttf");
  assert_eq!(
    glyph_name(font, font_glyph(font, Icon::Settings2.glyph() as usize)),
    "settings-2"
  );
}

/// The glyphs of a team task's Take over (`hand-helping`, which lucide named `helping-hand` before, in 02d `lpji3`) and
/// of a run on another computer (`monitor-smartphone`, `Last event` in 02 `nMUwg`).
#[test]
fn team_claim_icons_match_bundled_font_names() {
  let font = include_bytes!("../../assets/fonts/lucide.ttf");
  for (icon, names) in [
    (Icon::HandHelping, ["hand-helping", "helping-hand"]),
    (Icon::MonitorSmartphone, ["monitor-smartphone", "monitor-smartphone"]),
  ] {
    let codepoint = (0xe000..0xf000)
      .find(|codepoint| names.contains(&glyph_name(font, font_glyph(font, *codepoint))))
      .expect("the bundled font includes the icon");
    assert_eq!(icon.glyph() as usize, codepoint, "{icon:?} codepoint: {codepoint:x}");
  }
}

/// The glyphs of the Orchestrator's lines of events of tasks' runs and of the tag of what started a turn (ORC-779,
/// `c9YRv`, `OKtX2`, `q6udQx` in `Orchestrator watch · redesign`).
#[test]
fn task_event_icons_match_bundled_font_names() {
  let font = include_bytes!("../../assets/fonts/lucide.ttf");
  for (icon, name) in [
    (Icon::TimerOff, "timer-off"),
    (Icon::GitPullRequestArrow, "git-pull-request-arrow"),
    (Icon::GitMerge, "git-merge"),
    (Icon::Ban, "ban"),
    (Icon::Zap, "zap"),
  ] {
    assert_eq!(glyph_name(font, font_glyph(font, icon.glyph() as usize)), name);
  }
}

/// The glyphs of the `/watching` answer (ORC-775, 06 `Ff7AD`): its avatar and command chip, and a head row's target.
/// The bundled font names the design's `square-check` by its older name.
#[test]
fn watching_icons_match_bundled_font_names() {
  let font = include_bytes!("../../assets/fonts/lucide.ttf");
  for (icon, name) in [
    (Icon::ListTree, "list-tree"),
    (Icon::Boxes, "boxes"),
    (Icon::SquareCheck, "check-square-2"),
    (Icon::Inbox, "inbox"),
    (Icon::FolderKanban, "folder-kanban"),
  ] {
    let codepoint = (0xe000..0xf8ff)
      .find(|codepoint| glyph_name(font, font_glyph(font, *codepoint)) == name)
      .expect("the bundled font includes the icon");
    assert_eq!(icon.glyph() as usize, codepoint, "{icon:?} codepoint: {codepoint:x}");
  }
}

/// The glyphs of the lines the app writes about what an Orchestrator conversation watches and of a watch command's
/// module chip (ORC-777, `avDZO`, `mgpxA` in `OKtX2`).
#[test]
fn watch_line_icons_match_bundled_font_names() {
  let font = include_bytes!("../../assets/fonts/lucide.ttf");
  let boxes = (0xe000..0xf000)
    .find(|codepoint| glyph_name(font, font_glyph(font, *codepoint)) == "boxes")
    .expect("the bundled font includes boxes");
  assert_eq!(Icon::Boxes.glyph() as usize, boxes, "boxes codepoint: {boxes:x}");
  for (icon, name) in [
    (Icon::ArrowRightLeft, "arrow-right-left"),
    (Icon::EyeOff, "eye-off"),
    (Icon::Flag, "flag"),
    (Icon::Eye, "eye"),
    (Icon::ScanEye, "scan-eye"),
  ] {
    assert_eq!(glyph_name(font, font_glyph(font, icon.glyph() as usize)), name);
  }
}
