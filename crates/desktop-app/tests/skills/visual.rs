//! Visual contracts reproduced with actual private repository data, not fixture defaults.
use super::*;

#[test]
fn skills_visual_empty_variants_and_populated_repository_setting_are_truthful() {
  let mut s = Skills::new();
  s.go(ProjectPage::Skills);
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
  s.go(ProjectPage::Skills);
  s.window.until("populated library", |w| {
    w.tree.get_element_by_id("skills-list").is_some()
  });
  assert!(s.window.tree.get_element_by_id("skill-repository-files").is_some());
  assert!(s.text("skills-ownership").contains("Native agents"));
}

#[test]
fn skills_visual_import_paths_types_totals_and_returned_metadata_do_not_overlap() {
  let mut s = Skills::new();
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
  s.go(ProjectPage::Skills);
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
  for i in 1..=2 {
    let title = bounds(&mut s, &format!("skill-source-{i}"));
    let summary = bounds(&mut s, &format!("skill-source-summary-{i}"));
    let row = bounds(&mut s, &format!("skill-source-row-{i}"));
    let kind = bounds(&mut s, &format!("skill-source-type-{i}"));
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

#[test]
fn skills_visual_empty_description_explanation_and_saved_published_header_are_visible() {
  let mut s = Skills::new();
  let id = s.seed("checks", true);
  s.go(ProjectPage::Skill(id));
  assert_eq!(s.text("skill-publish"), "Publish");
  s.set("skill-description", "");
  s.settle(id, "");
  assert!(s.text("skill-save-status").contains("Draft saved · Published v1"));
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
