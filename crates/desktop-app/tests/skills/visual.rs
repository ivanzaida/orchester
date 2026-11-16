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
  let mut s = Skills::new();
  s.seed("checks", false);
  s.go(ProjectPage::Skills(Default::default()));
  s.window.until("populated library", |w| {
    w.tree.get_element_by_id("skills-group-project").is_some()
  });
  assert!(s.text("skills-count").contains("1 skill · 1 with unpublished drafts"));
  assert!(s.text("skills-group-project").contains("checks"));
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
fn skills_visual_a_skill_a_client_created_through_the_window_is_v1_and_holds_no_mark() {
  let mut s = Skills::new();
  s.go(ProjectPage::NewSkill);
  client_sets(&mut s, "skill-name", "client-checks");
  client_sets(&mut s, "skill-description", "A client's description.");
  client_sets(&mut s, "skill-body", "# Checks\nA client's body.");
  s.click("skill-create");
  s.window.until("created skill editor", |w| {
    matches!(w.navigation().route_now(), Route::Project(_, ProjectPage::Skill(_)))
  });
  let Route::Project(_, ProjectPage::Skill(id)) = s.window.navigation().route_now() else {
    panic!("editor route")
  };
  assert_eq!(s.entry(id).state.published().expect("created as v1").get(), 1);
  assert!(!marked(&s, id), "a new skill is v1: it has no draft to mark");
}

fn glyph(s: &mut Skills, id: &str) -> String {
  s.window
    .tree
    .get_element_by_id(id)
    .and_then(|e| e.text_content().map(str::to_owned))
    .expect("icon text")
}
