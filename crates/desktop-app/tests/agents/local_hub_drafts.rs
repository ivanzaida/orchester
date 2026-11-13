//! A local project's profiles, instructions and skills are edited in place with no drafts (decided by the person
//! 2026-11-09): a panel opens on the published version, unchanged, even when a draft of it is stored (an MCP client's);
//! only what is typed changes it, *Cancel* leaves the draft alone and *Save* publishes the fields over it.
//! `cargo test -p orchester-app --test agents local_hub_drafts`.

use orchester_app::routes::{HubFilter, HubView};
use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{
    AgentTargets, Draft, InjectionEvent, Instruction, InstructionScope, PromptText, ResourceBody, Skill, Slug,
    VersionNumber,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::agents::data;

use super::{
  harness::{Agents, changed},
  local_hub::open,
};

/// `published` made as `slug` and published as v1, with `drafted` stored as its draft on v1.
fn drafted_on_v1<B: ResourceBody>(agents: &Agents, slug: &str, (published, drafted): (&B, B)) -> ResourceId {
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let id = db
    .create_resource(
      Slug::new(slug).expect("slug"),
      Name::new(slug).expect("name"),
      published,
      Timestamp::now(),
    )
    .expect("created")
    .id();
  db.publish_draft::<B>(id, Timestamp::now()).expect("v1");
  let v1 = VersionNumber::new(1.try_into().expect("above zero"));
  db.save_draft(id, &Draft::new(drafted, Some(v1), Timestamp::now()))
    .expect("a draft on v1");
  id
}

/// Whether the list marks the row of `id` as holding edits that are not saved.
fn marked(agents: &mut Agents, id: ResourceId) -> bool {
  agents
    .text(&format!("local-hub-row-{id}-line"))
    .contains("Unsaved changes")
}

/// A few passes, so that what the panel says of its edits reaches the list.
fn settle(agents: &mut Agents) {
  for _ in 0..3 {
    agents.window.tree.tick_futures();
    agents.window.pass();
  }
}

fn skill(description: &str) -> Skill {
  Skill::new(
    description.into(),
    "Run them all.".into(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("skill")
}

fn rule(text: &str) -> Instruction {
  Instruction::new(
    PromptText::new(text).expect("prompt"),
    AgentTargets::All,
    [InjectionEvent::Start].into(),
    InstructionScope::Project,
  )
  .expect("instruction")
}

#[test]
fn local_hub_drafts_a_skill_opens_on_its_published_version_and_cancel_leaves_the_draft() {
  let mut agents = Agents::new(true);
  let id = drafted_on_v1(
    &agents,
    "checks",
    (&skill("Runs the checks."), skill("What an MCP client drafted.")),
  );
  open(&mut agents, HubView::new(HubFilter::Skills, Some(id)));
  agents.until("the panel", |s| s.exists("local-hub-skill-description-input"));
  settle(&mut agents);

  assert_eq!(agents.value("local-hub-skill-description-input"), "Runs the checks.");
  assert!(!marked(&mut agents, id), "the panel opens unchanged");

  agents.set("local-hub-skill-description-input", "Runs every check.");
  agents.until("changed", |s| marked(s, id));
  agents.click("local-hub-skill-cancel");
  agents.until("unchanged again", |s| !marked(s, id));
  assert_eq!(agents.value("local-hub-skill-description-input"), "Runs the checks.");
  let db = ProjectDb::open(&agents.root).expect("database");
  assert!(
    db.draft::<Skill>(id).expect("read").is_some(),
    "Cancel leaves the draft alone"
  );
}

#[test]
fn local_hub_drafts_an_instruction_opens_on_its_published_version_and_save_replaces_the_draft() {
  let mut agents = Agents::new(true);
  let id = drafted_on_v1(
    &agents,
    "style",
    (&rule("Use tabs."), rule("What an MCP client drafted.")),
  );
  open(&mut agents, HubView::new(HubFilter::Instructions, Some(id)));
  agents.until("the panel", |s| s.exists("local-hub-instruction-text-input"));
  settle(&mut agents);

  assert_eq!(agents.value("local-hub-instruction-text-input"), "Use tabs.");
  assert!(!marked(&mut agents, id), "the panel opens unchanged");

  agents.set("local-hub-instruction-text-input", "Use spaces.");
  agents.until("changed", |s| marked(s, id));
  agents.click("local-hub-instruction-save");
  agents.until("published as v2", |s| {
    let db = ProjectDb::open(&s.root).expect("database");
    db.latest_version::<Instruction>(id)
      .expect("read")
      .is_some_and(|version| version.number().get() == 2 && version.body() == &rule("Use spaces."))
  });
  let db = ProjectDb::open(&agents.root).expect("database");
  assert!(
    db.draft::<Instruction>(id).expect("read").is_none(),
    "Save replaces the draft"
  );
}

#[test]
fn local_hub_drafts_a_profile_opens_on_its_published_version_unchanged() {
  let mut agents = Agents::new(true);
  let id = agents.profile(true);
  let body = changed(&agents.listed().profile(id).expect("profile").profile);
  let mut db = ProjectDb::open(&agents.root).expect("database");
  data::save_draft(&mut db, id, body, Timestamp::now()).expect("a draft");
  drop(db);

  open(&mut agents, HubView::new(HubFilter::Profiles, Some(id)));
  agents.until("the panel", |s| s.exists("local-hub-profile-effort"));
  settle(&mut agents);

  assert!(!marked(&mut agents, id), "the panel opens unchanged");
}
