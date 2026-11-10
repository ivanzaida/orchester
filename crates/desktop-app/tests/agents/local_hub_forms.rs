//! A local instruction is made and edited with one form (L5 for a new one; its panel, edited in place, as A27 draws
//! an edit): the form that makes one draws the same fields, in the same order, as the panel of one the project holds.
//! `cargo test -p orchester-app --test agents local_hub_forms`.

use orchester_app::routes::{HubFilter, HubView};
use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{AgentTargets, InjectionEvent, Instruction, InstructionScope, PromptText, ResourceBody, Slug},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  local_hub::open,
  local_hub_create::{form_route, show_form},
};

/// `body` made as `slug` and published as v1.
fn published<B: ResourceBody>(agents: &Agents, slug: &str, body: &B) -> ResourceId {
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let id = db
    .create_resource(
      Slug::new(slug).expect("slug"),
      Name::new(slug).expect("name"),
      body,
      Timestamp::now(),
    )
    .expect("created")
    .id();
  db.publish_draft::<B>(id, Timestamp::now()).expect("v1");
  id
}

/// What the form's parts say, in order: each one's label, with *· required* (a new one's mark) left out.
fn labels(agents: &mut Agents, prefix: &str, parts: &[&str]) -> Vec<String> {
  parts
    .iter()
    .map(|part| {
      let id = format!("{prefix}-{part}-label");
      assert!(agents.exists(&id), "{id} is drawn");
      agents.text(&id).trim_end_matches(" · required").to_owned()
    })
    .collect()
}

#[test]
fn local_hub_forms_create_and_edit_draw_the_same_fields() {
  let mut agents = Agents::new(true);
  let rule = Instruction::new(
    PromptText::new("Use tabs.").expect("prompt"),
    AgentTargets::All,
    [InjectionEvent::Start].into(),
    InstructionScope::Project,
  )
  .expect("instruction");
  let kinds = [(
    HubFilter::Instructions,
    published(&agents, "style", &rule),
    "instruction",
    &["format", "text"][..],
  )];
  for (filter, id, name, parts) in kinds {
    open(&mut agents, HubView::new(filter, Some(id)));
    let panel = format!("local-hub-{name}");
    let first = format!("{panel}-{}-input", parts.last().expect("a part"));
    agents.until("the panel", |s| s.exists(&first));
    let edited = labels(&mut agents, &panel, parts);
    show_form(&mut agents, filter);
    assert_eq!(agents.window.navigation().route_now(), form_route(filter));
    let created = labels(&mut agents, &format!("team-new-{name}"), parts);
    assert_eq!(created, edited, "{name}: one form, the same fields in the same order");
  }
}
