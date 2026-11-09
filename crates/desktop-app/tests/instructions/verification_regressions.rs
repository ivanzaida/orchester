//! Major findings returned by Verify attempt 12, exercised in the real screen.
use lurq::{
  app::theme::TypographyStyle,
  layout::{quad::QuadContent, text_style::FontWeight},
};
use orchester_core::resource::InstructionScope;

use super::{harness::Instructions, preview::seed_preview};

#[test]
fn instructions_verify_preview_label_tracks_draft_and_published_state() {
  let mut screen = Instructions::new();
  seed_preview(&screen);
  let id = screen.rule(
    "Standards",
    "Working on {{task.title}}",
    InstructionScope::Project,
    false,
  );
  screen.go();
  screen.click("instruction-panel-preview");
  screen.until("draft preview", |s| s.exists("instruction-full-prompt"));
  assert!(
    screen
      .text("instruction-panel")
      .contains("Draft preview · a run started now")
  );
  screen.click("instruction-publish");
  screen.until("published preview", |s| {
    s.text("instruction-panel").contains("Published rules only")
  });
  assert!(!screen.entry(id).state.has_draft());
  let published = screen.text("instruction-panel");
  assert!(published.contains("Preview · a run started now"));
  assert!(!published.contains("Draft preview"));
  screen.click("instruction-full-prompt");
  assert!(!screen.text("instruction-full-viewer").contains("DRAFT PREVIEW"));
  screen.click("instruction-viewer-back");
  screen.set("instruction-prompt", "Edited {{task.title}}");
  screen.until("edited draft saved", |s| s.entry(id).state.has_draft());
  assert!(
    screen
      .text("instruction-panel")
      .contains("Draft preview · a run started now")
  );
}

#[test]
fn instructions_verify_full_viewer_heading_uses_design_title_typography() {
  let mut screen = Instructions::new();
  seed_preview(&screen);
  screen.rule(
    "Standards",
    "Working on {{task.title}}",
    InstructionScope::Project,
    true,
  );
  screen.go();
  screen.click("instruction-panel-preview");
  screen.until("preview", |s| s.exists("instruction-full-prompt"));
  screen.click("instruction-full-prompt");
  let style = screen.window.app.theme().typography_style(TypographyStyle::Title);
  assert_eq!(
    (style.font_size, style.weight, &*style.font_family),
    (15.0, FontWeight::SemiBold, "Geist")
  );
  let painted = screen.window.tree.painted_quads();
  let rendered = painted
    .iter()
    .find_map(|quad| match &quad.content {
      QuadContent::Text { text, style, .. } if text == "Effective instructions" => Some(style),
      _ => None,
    })
    .expect("painted viewer heading");
  assert!(
    rendered == &style,
    "the rendered heading must use the registered Title role"
  );
  assert_eq!(screen.text("instruction-viewer-crumbs-2"), "Effective instructions");
}
