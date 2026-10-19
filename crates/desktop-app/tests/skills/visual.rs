//! Visual contracts reproduced with actual private repository data, not fixture defaults.
use super::*;

#[test]
fn skills_visual_empty_variants_and_populated_repository_setting_are_truthful() {
  let mut s = Skills::new();
  s.go(ProjectPage::Skills(Default::default()));
  assert!(!s.text("skills-empty").contains("Found in this repository"));
  assert!(s.text("skills-empty").contains("Write the first one for Kontur"));
  assert!(
    s.text("skills-empty")
      .contains("Starts an empty draft for all of Kontur")
  );
  assert!(s.window.tree.get_element_by_id("skills-found").is_none());
  let card = bounds(&mut s, "skills-empty");
  assert!(
    card.height >= 216.0 && card.height < 300.0,
    "first-skill card keeps its own height: {card:?}"
  );
  let guide = bounds(&mut s, "skills-guidance");
  let ownership = bounds(&mut s, "skills-ownership");
  assert!(ownership.y >= guide.y + guide.height && ownership.x >= guide.x);
  let mut s = Skills::new();
  s.seed("checks", false);
  s.go(ProjectPage::Skills(Default::default()));
  s.window.until("populated library", |w| {
    w.tree.get_element_by_id("skills-list").is_some()
  });
  assert!(s.window.tree.get_element_by_id("skill-repository-files").is_some());
  assert!(s.text("skills-ownership").contains("Native agents"));
}

#[test]
fn skills_visual_import_paths_types_totals_and_returned_metadata_do_not_overlap() {
  let mut s = Skills::new();
  let reference = prepare_import(&s);
  s.go(ProjectPage::Skills(Default::default()));
  assert!(s.text("skills-found").contains("3 items"));
  s.click("skills-import-open");
  assert!(s.text("skills-import-review").contains("1 instruction, 2 skills"));
  assert!(s.text("skill-import-create").contains("Create 3 drafts"));
  assert!(s.text("skill-source-type-0").contains("Instruction"));
  assert!(s.text("skill-source-type-1").contains("Skill"));
  s.click("skill-import-select-2");
  assert!(s.text("skills-import-review").contains("1 instruction, 1 skill"));
  assert!(s.text("skill-import-create").contains("Create 2 drafts"));
  s.click("skill-import-select-2");
  s.window.pass();
  check_review_geometry(&mut s);
  s.click("skill-import-create");
  s.window.until("actual import result", |w| {
    w.tree.get_element_by_id("skills-import-result").is_some()
  });
  assert!(
    s.text("skills-import-result")
      .contains("CREATED · 1 INSTRUCTION, 2 SKILLS")
  );
  assert!(s.text("skills-import-result").contains("Draft skill v1 · 1 file"));
  assert!(s.text("skills-import-result").contains("No agent gets a draft"));
  assert!(s.text("skills-import-result").contains("Review skills"));
  assert!(
    s.text("skill-result-footer")
      .contains("3 of 3 items imported · 1 instruction, 2 skills")
  );
  let result = bounds(&mut s, "skills-import-result");
  let first = bounds(&mut s, "skill-created-row-0");
  assert!(
    (first.x - result.x - 24.0).abs() < 0.1,
    "created list has the designed body inset: {first:?}"
  );
  let footer = bounds(&mut s, "skill-result-footer");
  assert!((footer.y + footer.height - result.y - result.height).abs() < 0.1);
  for i in 1..=2 {
    let name = bounds(&mut s, &format!("skill-created-name-{i}"));
    let source = bounds(&mut s, &format!("skill-created-source-{i}"));
    let row = bounds(&mut s, &format!("skill-created-row-{i}"));
    assert!(name.width > 200.0 && source.width > 200.0);
    assert!(source.y >= name.y + name.height - 0.1, "name and origin do not overlap");
    assert!(
      row.y + row.height >= source.y + source.height - 0.1,
      "row contains the full origin"
    );
  }
  assert_eq!(
    fs::read_to_string(reference).expect("unchanged source"),
    "Private reference content"
  );
  assert!(
    ProjectSkills::load_in(&s.repo)
      .expect("actual storage")
      .entries
      .iter()
      .all(|e| e.state.published().is_none())
  );
}

/// WMT1W `r3tdmV` / `WYErT`: the header centers its contents in 40 with room above them, the
/// footer's explanation is inset 10/12 inside its own band, and a pointer-only source has no arrow.
#[test]
fn skills_visual_found_header_footer_insets_and_pointer_row() {
  let mut s = Skills::new();
  prepare_import(&s);
  fs::write(s.repo.join("CLAUDE.md"), "@AGENTS.md\n").expect("pointer");
  s.go(ProjectPage::Skills(Default::default()));
  assert!(s.text("skills-found").contains("4 items"));
  let found = bounds(&mut s, "skills-found");
  let header = bounds(&mut s, "skills-found-header");
  let title = bounds(&mut s, "skills-found-title");
  let folder = bounds(&mut s, "skills-found-folder");
  // The fixture's folder is long enough to wrap: the header grows around it instead of cutting it.
  assert!(folder.height > 20.0, "long folder wraps: {folder:?}");
  assert!(
    header.height >= 40.0 && folder.y - header.y >= 9.9 && header.y + header.height - folder.y - folder.height >= 9.9,
    "header keeps room around the whole folder: {header:?} / {folder:?}"
  );
  assert!(
    (folder.y + folder.height / 2.0 - (header.y + header.height / 2.0)).abs() < 1.0,
    "folder centered: {folder:?} in {header:?}"
  );
  let middle = header.y + header.height / 2.0;
  assert!(
    (title.y + title.height / 2.0 - middle).abs() < 1.0,
    "title centered: {title:?} in {header:?}"
  );
  assert!(title.y - header.y >= 10.0, "room above the title: {title:?}");
  assert!(
    folder.x + folder.width <= header.x + header.width - 11.9,
    "folder ends at the header's inset: {folder:?}"
  );
  let footer = bounds(&mut s, "skills-found-footer");
  let note = bounds(&mut s, "skills-found-note");
  assert!(
    s.text("skills-found-note")
      .contains("Import from repository shows each item")
  );
  assert!(
    (note.x - footer.x - 12.0).abs() < 0.1,
    "explanation inset from the left: {note:?} in {footer:?}"
  );
  assert!(
    note.y - footer.y >= 10.0 && note.y - footer.y < 11.5,
    "explanation inset from the top: {note:?} in {footer:?}"
  );
  assert!(footer.y + footer.height <= found.y + found.height + 0.1);
  assert!(
    found.y + found.height - footer.y - footer.height < 1.1,
    "footer closes the box"
  );
  assert!(s.text("skill-source-1").contains("CLAUDE.md"));
  assert!(s.text("skill-source-summary-1").contains("nothing to import"));
  assert!(s.window.tree.get_element_by_id("skill-source-arrow-1").is_none());
  assert!(s.window.tree.get_element_by_id("skill-source-arrow-0").is_some());
  let long = bounds(&mut s, "skill-source-3");
  let row = bounds(&mut s, "skill-source-row-3");
  assert!(s.text("skill-source-3").contains("long-supported-repository-name"));
  assert!(
    row.y + row.height >= long.y + long.height - 0.1,
    "row holds the wrapped source"
  );
}

#[test]
fn skills_visual_empty_description_explanation_and_saved_published_header_are_visible() {
  let mut s = Skills::new();
  let id = s.seed("checks", true);
  s.go(ProjectPage::Skill(id));
  assert_eq!(s.text("skill-publish"), "Publish");
  s.set("skill-description", "");
  s.settle(id, "");
  assert!(s.text("skill-save-status").contains("Draft saved · Published v1"));
  assert_eq!(
    glyph(&mut s, "skill-save-icon"),
    orchester_app::ui::icon::Icon::CloudCheck.glyph().to_string()
  );
  assert!(s.text("skill-publish-unavailable").contains("description"));
  let reason = bounds(&mut s, "skill-publish-unavailable");
  let description = bounds(&mut s, "skill-description");
  assert!(reason.height > 0.0 && description.y >= reason.y + reason.height);
  assert_eq!(s.entry(id).state.published().expect("retained version").get(), 1);
}

fn bounds(s: &mut Skills, id: &str) -> lurq::core::ElementRect {
  s.window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|e| e.bounds())
    .unwrap_or_else(|| panic!("{id} measured"))
}

#[test]
fn skills_visual_unpublished_status_and_disabled_upload_keep_actual_draft() {
  use orchester_app::ui::icon::Icon;
  let mut s = Skills::new();
  let id = s.seed("checks", false);
  s.go(ProjectPage::Skill(id));
  assert!(s.text("skill-save-status").contains("Draft saved · Not published"));
  assert_eq!(glyph(&mut s, "skill-save-icon"), Icon::FilePenLine.glyph().to_string());
  s.set("skill-name", "");
  assert!(s.text("skill-save-status").contains("fix marked fields"));
  assert_eq!(glyph(&mut s, "skill-publish-icon"), Icon::Upload.glyph().to_string());
  s.click("skill-publish");
  assert!(s.window.tree.get_element_by_id("skill-confirm-confirm").is_none());
  assert_eq!(s.entry(id).name.as_ref(), "checks");
  assert!(s.entry(id).state.published().is_none());
}

fn glyph(s: &mut Skills, id: &str) -> String {
  s.window
    .tree
    .get_element_by_id(id)
    .and_then(|e| e.text_content().map(str::to_owned))
    .expect("icon text")
}
fn prepare_import(s: &Skills) -> PathBuf {
  let names = [
    "write-migration",
    "write-migration-for-a-long-supported-repository-name",
  ];
  for name in names {
    let folder = s.repo.join(".claude/skills").join(name);
    fs::create_dir_all(&folder).expect("private skill folder");
    fs::write(
      folder.join("SKILL.md"),
      format!("---\nname: {name}\ndescription: Migrate storage\n---\n# Migration\nRead the plan."),
    )
    .expect("source");
  }
  fs::write(s.repo.join("AGENTS.md"), "Read the migration plan.\nThen run checks.").expect("instruction");
  let reference = s.repo.join(".claude/skills/write-migration/notes.md");
  fs::write(&reference, "Private reference content").expect("reference");
  reference
}
fn check_review_geometry(s: &mut Skills) {
  for i in 1..=2 {
    let title = bounds(s, &format!("skill-source-{i}"));
    let summary = bounds(s, &format!("skill-source-summary-{i}"));
    let row = bounds(s, &format!("skill-source-row-{i}"));
    let kind = bounds(s, &format!("skill-source-type-{i}"));
    assert!(row.height >= 50.0, "rows grow from the ordinary 50px design height");
    assert!(title.width > 200.0, "source keeps its allocated width: {title:?}");
    assert!(
      summary.y >= title.y + title.height - 0.1,
      "source and summary do not overlap: {title:?} / {summary:?}"
    );
    assert!(
      row.y + row.height >= summary.y + summary.height - 0.1,
      "row contains wrapped summary"
    );
    assert!(kind.x >= title.x + title.width - 0.1, "type is a separate column");
  }
  let notice = bounds(s, "skill-import-notice");
  let footer = bounds(s, "skill-import-footer");
  let review = bounds(s, "skills-import-review");
  assert!(
    footer.y >= notice.y + notice.height,
    "consequences precede the final action band"
  );
  assert!(
    (footer.y + footer.height - review.y - review.height).abs() < 0.1,
    "footer is the final band"
  );
}
