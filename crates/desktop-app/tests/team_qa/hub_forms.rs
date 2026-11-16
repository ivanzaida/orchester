//! A team instruction, skill and tool are made and edited with one form each (`Agents · v2` A10–A12 for a new one,
//! A27–A29 for an admin's edit): the form that makes one draws the same fields, in the same order, as the admin's edit
//! of one the team holds. On the QA state `team-edit`, against the stand-in team server.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_hub_forms`

use orchester_app::{
  routes::{HubFilter, HubView, ProjectPage, Route},
  test_support::Window,
};
use orchester_core::{resource::ResourceKind, store::ResourceStore};

use super::{
  hub_edit::{db, opened},
  team::State,
};

/// One kind's form: its filter, its kind and a resource of it the seed holds, the part of the create form's ids after
/// `team-new-`, and its parts: the rows' and the sections'.
struct Kind {
  filter: HubFilter,
  kind: ResourceKind,
  seeded: &'static str,
  name: &'static str,
  rows: &'static [&'static str],
  sections: &'static [&'static str],
}

const KINDS: [Kind; 3] = [
  Kind {
    filter: HubFilter::Instructions,
    kind: ResourceKind::Instruction,
    seeded: "Project rules",
    name: "instruction",
    rows: &["format"],
    sections: &["text"],
  },
  Kind {
    filter: HubFilter::Skills,
    kind: ResourceKind::Skill,
    seeded: "review-diff",
    name: "skill",
    rows: &["description", "files"],
    sections: &["skill-md"],
  },
  Kind {
    filter: HubFilter::Tools,
    kind: ResourceKind::Tool,
    seeded: "cargo-check",
    name: "tool",
    rows: &["kind", "description", "program", "arguments", "timeout"],
    sections: &[],
  },
];

/// What the form's parts say, in order: each row's label, and each section's, with *· required* (a new one's mark)
/// left out; and whether each takes typing (a field) or shows a value.
fn parts(window: &mut Window, kind: &Kind, (rows, sections): (&str, &str)) -> Vec<(String, bool)> {
  let rows = kind.rows.iter().map(|part| format!("{rows}-{part}"));
  let sections = kind.sections.iter().map(|part| format!("{sections}-{part}"));
  rows
    .chain(sections)
    .map(|id| {
      let label = window
        .texts(&format!("{id}-label"))
        .unwrap_or_else(|| panic!("{id} is drawn"))
        .join(" ");
      let typed = window.texts(&format!("{id}-input")).is_some();
      (label.trim_end_matches(" · required").to_owned(), typed)
    })
    .collect()
}

#[test]
fn team_qa_hub_forms_create_and_edit_draw_the_same_fields() {
  for kind in &KINDS {
    let name = kind.seeded.to_owned();
    let resource = kind.kind;
    let (mut window, _server, (project, picked)) =
      opened(("team-edit", State::TeamEdit), "team-hub-panel-title", move |window| {
        let id = db(window)
          .resources(resource)
          .expect("resources")
          .into_iter()
          .find(|resource| resource.name().as_str() == name)
          .expect("the seed's resource")
          .id();
        (HubView::new(kind.filter, Some(id)), Some(id))
      });
    assert!(picked.is_some());
    window.until("the admin's edit", |window| {
      window.texts("team-hub-edit-save").is_some()
    });
    let edited = parts(&mut window, kind, ("team-hub-from-team", "team-hub"));
    window
      .navigation()
      .go(Route::Project(project, ProjectPage::hub(HubView::adding(kind.filter))));
    window.until("the form", |window| window.texts("team-hub-new-create").is_some());
    let prefix = format!("team-new-{}", kind.name);
    let created = parts(&mut window, kind, (&prefix, &prefix));
    assert_eq!(
      created, edited,
      "{}: one form, the same fields in the same order",
      kind.name
    );
    assert!(
      window.texts(&format!("{prefix}-name-input")).is_some(),
      "{}: a new one is named",
      kind.name
    );
  }
}
