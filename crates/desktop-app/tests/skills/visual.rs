//! Visual contracts reproduced with actual private repository data, not fixture defaults.
use super::*;

#[test]
fn skills_visual_empty_list_and_populated_groups_are_truthful() {
  let mut s = Skills::new();
  s.go(ProjectPage::Skills(Default::default()));
  assert!(!s.text("skills-empty").contains("Found in this repository"));
  assert!(s.text("skills-empty").contains("No skills in Kontur yet"));
  assert!(s.text("skills-empty").contains("Write the first one for Kontur"));
  assert!(s.window.tree.get_element_by_id("skills-empty-new").is_some());
  assert!(s.window.tree.get_element_by_id("skill-repository-files").is_some());
  let mut s = Skills::new();
  s.seed("checks", false);
  s.go(ProjectPage::Skills(Default::default()));
  s.window.until("populated library", |w| {
    w.tree.get_element_by_id("skills-group-project").is_some()
  });
  assert!(s.text("skills-count").contains("1 skill · 1 with unpublished drafts"));
  assert!(s.text("skills-group-project").contains("checks"));
  assert!(s.text("skills-repository-settings").contains("Native agents"));
}

#[test]
fn skills_visual_import_paths_types_totals_and_returned_metadata_do_not_overlap() {
  let mut s = Skills::new();
  let reference = prepare_import(&s);
  s.go(ProjectPage::Skills(Default::default()));
  assert!(s.text("skills-repository-settings").contains("3 items"));
  s.click("skills-import-open");
  assert!(s.text("skills-import-review").contains("1 instruction, 2 skills"));
  assert!(s.text("skill-import-create").contains("Create 3 drafts"));
  // Grouped by kind as 02c draws them: each group says what its items become.
  assert!(
    s.text("skill-import-instructions")
      .contains("each becomes a draft project instruction")
  );
  assert!(s.text("skill-import-instructions").contains("AGENTS.md"));
  assert!(
    s.text("skill-import-skills")
      .contains("Skills · each becomes a draft skill")
  );
  s.click("skill-import-toggle-2");
  assert!(s.text("skills-import-review").contains("1 instruction, 1 skill"));
  assert!(s.text("skill-import-create").contains("Create 2 drafts"));
  s.click("skill-import-toggle-2");
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

/// *Found in this repository* (`03`): what the repository holds, counted, with the switch and Import; the import
/// page lists each item, a pointer-only source with nothing to import.
#[test]
fn skills_visual_found_in_this_repository_counts_and_opens_the_import() {
  let mut s = Skills::new();
  prepare_import(&s);
  fs::write(s.repo.join("CLAUDE.md"), "@AGENTS.md\n").expect("pointer");
  s.go(ProjectPage::Skills(Default::default()));
  assert!(s.text("skills-repository-settings").contains("4 items"));
  assert!(
    s.text("skills-found-import")
      .contains("Import shows each item with a preview")
  );
  s.click("skills-found-import-open");
  s.window.until("the import page", |w| {
    w.tree.get_element_by_id("skills-import-review").is_some()
  });
  assert!(s.text("skill-import-path-1").contains("CLAUDE.md"));
  assert!(s.text("skill-import-detail-1").contains("nothing to import"));
  // A source with nothing to import is not chosen: three of the four items become drafts.
  assert!(s.text("skill-import-create").contains("Create 3 drafts"));
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
    orchester_app::ui::icon::Icon::CircleDashed.glyph().to_string()
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
  assert_eq!(glyph(&mut s, "skill-save-icon"), Icon::CircleDashed.glyph().to_string());
  s.set("skill-name", "");
  assert!(s.text("skill-save-status").contains("fix marked fields"));
  assert_eq!(glyph(&mut s, "skill-publish-icon"), Icon::Upload.glyph().to_string());
  s.click("skill-publish");
  assert!(s.window.tree.get_element_by_id("skill-confirm-confirm").is_none());
  assert_eq!(s.entry(id).name.as_ref(), "checks");
  assert!(s.entry(id).state.published().is_none());
}

/// Saves `description` over skill `id`'s draft as `save_skill_draft` does: marked as automation's (ORC-79).
fn automation_saves(s: &Skills, id: ResourceId, description: &str) {
  let entry = s.entry(id);
  let body = Skill::new(
    description.into(),
    entry.body.body().to_owned(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("skill");
  let now = orchester_core::time::Timestamp::now();
  let saved = editor::save_checked_in(&s.repo, (Some(&entry), &entry.name, &body), now).expect("automation's draft");
  editor::mark_automation_in(&s.repo, &saved, now).expect("marked");
}

#[test]
fn skills_visual_mark_a_draft_automation_wrote_until_a_person_saves_it() {
  use orchester_app::ui::icon::Icon;
  let mut s = Skills::new();
  let id = s.seed("checks", true);
  automation_saves(&s, id, "Automation's description.");
  s.go(ProjectPage::Skill(id));
  s.window.until("the automation mark", |w| {
    w.texts("skill-save-status")
      .is_some_and(|t| t.join(" ") == "Draft written by automation · Published v1")
  });
  assert_eq!(glyph(&mut s, "skill-save-icon"), Icon::CircleDashed.glyph().to_string());
  s.set("skill-description", "A person's description.");
  s.settle(id, "A person's description.");
  s.window.until("the person's draft", |w| {
    w.texts("skill-save-status")
      .is_some_and(|t| t.join(" ") == "Draft saved · Published v1")
  });
  assert_eq!(glyph(&mut s, "skill-save-icon"), Icon::CircleDashed.glyph().to_string());
  let skills = ProjectSkills::load_in(&s.repo).expect("read");
  assert!(
    !skills.by_automation(&s.entry(id)),
    "a person's save makes the draft theirs"
  );
}

#[test]
fn skills_visual_mark_a_new_skill_automation_wrote() {
  let mut s = Skills::new();
  let id = s.seed("checks", false);
  automation_saves(&s, id, "Automation's description.");
  s.go(ProjectPage::Skill(id));
  s.window.until("the automation mark", |w| {
    w.texts("skill-save-status")
      .is_some_and(|t| t.join(" ") == "Draft written by automation · Not published")
  });
}

/// Types `value` into `id` as an MCP client's `lurq_set_value` does: lurq delivers it as synthetic input, and the
/// editor saves it on a later pass, outside that input.
fn client_sets(s: &mut Skills, id: &str, value: &str) {
  lurq::app::events::with_synthetic_input(|| {
    s.window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|e| e.as_text_input())
      .unwrap_or_else(|| panic!("{id} input"))
      .set_value(value);
  });
  s.window.pass();
}

fn marked(s: &Skills, id: ResourceId) -> bool {
  ProjectSkills::load_in(&s.repo)
    .expect("read")
    .by_automation(&s.entry(id))
}

#[test]
fn skills_visual_mark_a_draft_a_client_saved_through_the_window() {
  let mut s = Skills::new();
  let id = s.seed("checks", true);
  s.go(ProjectPage::Skill(id));
  client_sets(&mut s, "skill-description", "A client's description.");
  s.settle(id, "A client's description.");
  s.window.until("the automation mark", |w| {
    w.texts("skill-save-status")
      .is_some_and(|t| t.join(" ") == "Draft written by automation · Published v1")
  });
  assert!(marked(&s, id), "a client's window save is automation's");
  s.set("skill-description", "A person's description.");
  s.settle(id, "A person's description.");
  s.window.until("the person's draft", |w| {
    w.texts("skill-save-status")
      .is_some_and(|t| t.join(" ") == "Draft saved · Published v1")
  });
  assert!(!marked(&s, id), "the person's next save makes it theirs");
}

#[test]
fn skills_visual_a_client_saving_over_an_automation_draft_keeps_the_mark() {
  let mut s = Skills::new();
  let id = s.seed("checks", false);
  automation_saves(&s, id, "Automation's description.");
  s.go(ProjectPage::Skill(id));
  s.window.until("the automation mark", |w| {
    w.texts("skill-save-status")
      .is_some_and(|t| t.join(" ") == "Draft written by automation · Not published")
  });
  client_sets(&mut s, "skill-body", "# Checks\nA client's body.");
  let root = s.repo.clone();
  s.window.until("the client's draft saved", move |_| {
    ProjectSkills::load_in(&root)
      .expect("read")
      .entry(id)
      .is_some_and(|e| e.body.body() == "# Checks\nA client's body.")
  });
  s.window.until("the mark stays", |w| {
    w.texts("skill-save-status")
      .is_some_and(|t| t.join(" ") == "Draft written by automation · Not published")
  });
  assert!(marked(&s, id), "a client's save does not clear the mark");
}

#[test]
fn skills_visual_a_skill_a_client_created_through_the_window_is_marked() {
  let mut s = Skills::new();
  s.go(ProjectPage::NewSkill);
  client_sets(&mut s, "skill-name", "client-checks");
  s.click("skill-create");
  s.window.until("created skill editor", |w| {
    matches!(w.navigation().route_now(), Route::Project(_, ProjectPage::Skill(_)))
  });
  let Route::Project(_, ProjectPage::Skill(id)) = s.window.navigation().route_now() else {
    panic!("editor route")
  };
  assert!(marked(&s, id), "a skill a client named is automation's draft");
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
/// 02c's layout: the notice above the groups, rows holding their path over what it becomes, the bar last.
fn check_review_geometry(s: &mut Skills) {
  for i in 1..=2 {
    let title = bounds(s, &format!("skill-import-path-{i}"));
    let summary = bounds(s, &format!("skill-import-detail-{i}"));
    let row = bounds(s, &format!("skill-import-item-{i}"));
    assert!(row.height >= 50.0, "rows keep the design's 58px rows: {row:?}");
    assert!(
      summary.y >= title.y + title.height - 0.1,
      "source and summary do not overlap: {title:?} / {summary:?}"
    );
    assert!(
      row.y + row.height >= summary.y + summary.height - 0.1,
      "row contains wrapped summary"
    );
  }
  let notice = bounds(s, "skill-import-notice");
  let groups = bounds(s, "skill-import-instructions");
  let footer = bounds(s, "skill-import-footer");
  let review = bounds(s, "skills-import-review");
  assert!(groups.y >= notice.y + notice.height, "the notice comes first");
  assert!(
    footer.y >= groups.y + groups.height,
    "the groups precede the final action band"
  );
  assert!(
    (footer.y + footer.height - review.y - review.height).abs() < 0.1,
    "the bar is the column's last part"
  );
}
