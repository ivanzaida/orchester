//! The four major findings returned by Verify attempt 12, exercised in the real screen.
use lurq::{
  app::theme::TypographyStyle,
  core::ElementRect,
  layout::{
    quad::QuadContent,
    text_style::{FontWeight, VerticalAlign},
  },
};
use orchester_core::resource::InstructionScope;

use super::{harness::Instructions, preview::seed_preview};

fn bounds(screen: &mut Instructions, id: &str) -> ElementRect {
  screen
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} laid out"))
}

#[test]
fn instructions_verify_import_preview_has_nonempty_content_inside_the_viewport() {
  let mut screen = Instructions::new();
  let source = format!("# Agent Instructions\n\nKeep changes small.\n{}", "\n".repeat(45));
  std::fs::write(screen.root.join("AGENTS.md"), &source).expect("private source");
  screen.go();
  screen.click("instructions-import");
  screen.until("review", |s| s.exists("instruction-import-create"));
  screen.click("instruction-import-item-0");
  let text = screen.text("instruction-import-source-text");
  assert!(!text.trim().is_empty());
  assert!(text.starts_with("# Agent Instructions"));
  let viewport = bounds(&mut screen, "instruction-import-source");
  let content = bounds(&mut screen, "instruction-import-source-text");
  assert!(viewport.height > 100.0 && content.height > 0.0);
  assert!(
    content.y >= viewport.y && content.y < viewport.y + viewport.height,
    "first source lines must start in the visible viewport: {viewport:?} {content:?}"
  );
  let painted = screen.window.tree.painted_quads();
  let source = painted
    .iter()
    .find(|quad| {
      matches!(&quad.content,
    QuadContent::Text { text, .. } if text.starts_with("# Agent Instructions"))
    })
    .expect("nonempty source is painted");
  let QuadContent::Text { style, .. } = &source.content else {
    unreachable!()
  };
  assert!(
    style.vertical_align == VerticalAlign::LineBox,
    "source starts at its first line box"
  );
  assert!(
    source.y >= source.clip.y && source.y < source.clip.y + source.clip.height,
    "the source starts inside its paint clip"
  );
}

#[test]
fn instructions_verify_skill_path_and_subtitle_bounds_do_not_overlap() {
  let mut screen = Instructions::new();
  screen.window.tree.set_scale_factor(1.5);
  screen.window.tree.resize(2160, 1350);
  std::fs::write(screen.root.join("AGENTS.md"), "Keep changes small.\n").expect("private source");
  std::fs::write(screen.root.join("CLAUDE.md"), "@AGENTS.md\n").expect("private pointer");
  for (folder, name) in [
    (".claude/skills", "run-checks"),
    (".claude/skills", "write-migration"),
    (".agents/skills", "github-oauth"),
    (".agents/skills", "old-deploy"),
  ] {
    let path = screen.root.join(folder).join(name);
    std::fs::create_dir_all(&path).expect("private skill");
    std::fs::write(
      path.join("SKILL.md"),
      format!("---\nname: {name}\ndescription: Private skill\n---\nKeep changes small.\n"),
    )
    .expect("private skill source");
  }
  screen.go();
  screen.click("instructions-import");
  screen.until("review", |s| s.exists("instruction-import-create"));
  let index = (0..6)
    .find(|index| screen.text(&format!("instruction-import-path-{index}")) == ".claude/skills/write-migration")
    .expect("migration row");
  let row = bounds(&mut screen, &format!("instruction-import-item-{index}"));
  let path = bounds(&mut screen, &format!("instruction-import-path-{index}"));
  let detail = bounds(&mut screen, &format!("instruction-import-detail-{index}"));
  assert!(
    detail.y >= path.y + path.height,
    "path and subtitle: {path:?} {detail:?}"
  );
  assert!(
    path.height < 20.0,
    "the design's write-migration path fits one line: {path:?}"
  );
  assert!(detail.y + detail.height <= row.y + row.height, "row contains subtitle");
}

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
      .contains("DRAFT PREVIEW · A RUN STARTED NOW")
  );
  screen.click("instruction-publish");
  screen.until("published preview", |s| {
    s.text("instruction-panel").contains("Published rules only")
  });
  assert!(!screen.entry(id).state.has_draft());
  let published = screen.text("instruction-panel");
  assert!(published.contains("PREVIEW · A RUN STARTED NOW"));
  assert!(!published.contains("DRAFT PREVIEW"));
  screen.click("instruction-full-prompt");
  assert!(!screen.text("instruction-full-viewer").contains("DRAFT PREVIEW"));
  screen.click("instruction-viewer-back");
  screen.set("instruction-prompt", "Edited {{task.title}}");
  screen.until("edited draft saved", |s| s.entry(id).state.has_draft());
  assert!(
    screen
      .text("instruction-panel")
      .contains("DRAFT PREVIEW · A RUN STARTED NOW")
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
