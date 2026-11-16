//! A flow card's layout is the same in every state (D-022): hovering or focusing it shows its handle and ⋯ in room the
//! card keeps for them, so its title and its second line keep their place, and a version shown read-only puts its text
//! where the editable card puts it. `cargo test -p orchester-app --test agents pipeline_reorder`.

use orchester_core::resource::StepKey;

use super::{
  harness::Agents,
  pipeline_flow::open,
  pipeline_reorder::{card, flow, focus, hover},
  pipeline_step::keys,
  pipeline_versions::{open_list, two_versions},
};

/// Where an element stands: its left edge, top and width.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Place {
  x: f32,
  y: f32,
  width: f32,
}

fn place(agents: &mut Agents, id: &str) -> Place {
  let found = agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|found| found.bounds())
    .unwrap_or_else(|| panic!("{id} laid out"));
  Place {
    x: found.x,
    y: found.y,
    width: found.width,
  }
}

/// The card's own box, its title and its second line.
fn parts(agents: &mut Agents, key: &StepKey) -> [Place; 3] {
  let id = card(key);
  [
    place(agents, &id),
    place(agents, &format!("{id}-name")),
    place(agents, &format!("{id}-runs")),
  ]
}

fn away(agents: &mut Agents) {
  hover(agents, "pipeline-hub-steps-label");
}

#[test]
fn pipeline_reorder_a_card_keeps_its_layout_across_rest_hover_focus_and_rest() {
  let mut agents = Agents::new(true);
  let (id, [research, plan, _, checks, review]) = flow(&agents, false);
  open(&mut agents, id);

  // A card on the line, a card in a group with its number.
  for key in [&plan, &checks, &review] {
    away(&mut agents);
    let rest = parts(&mut agents, key);
    assert!(!agents.exists(&format!("{}-handle", card(key))), "no handle at rest");

    hover(&mut agents, &card(key));
    agents.until("the handle and ⋯", |s| {
      s.exists(&format!("{}-handle", card(key))) && s.exists(&format!("{}-more", card(key)))
    });
    assert_eq!(parts(&mut agents, key), rest, "{key}: hovering moves nothing");

    away(&mut agents);
    focus(&mut agents, key);
    agents.until("the keys' card shows them", |s| {
      s.exists(&format!("{}-more", card(key)))
    });
    assert_eq!(parts(&mut agents, key), rest, "{key}: focusing moves nothing");

    focus(&mut agents, &research);
    away(&mut agents);
    agents.until("back to rest", |s| !s.exists(&format!("{}-more", card(key))));
    assert_eq!(parts(&mut agents, key), rest, "{key}: back at rest it is where it was");
  }
}

#[test]
fn pipeline_reorder_a_version_shown_read_only_puts_its_text_where_the_editable_card_does() {
  let (mut agents, ready) = two_versions();
  let [plan, _] = <[StepKey; 2]>::try_from(keys(&agents, ready)).expect("two steps");
  let editable = parts(&mut agents, &plan);

  open_list(&mut agents);
  agents.click("pipeline-hub-version-1");
  agents.until("the version", |s| s.exists("pipeline-hub-version-viewing"));
  let read_only = parts(&mut agents, &plan);

  let (name, runs) = (1, 2);
  for (which, what) in [(name, "title"), (runs, "second line")] {
    assert_eq!(
      read_only[which].x, editable[which].x,
      "the {what} of a read-only card starts where the editable one does"
    );
  }
  assert_eq!(
    read_only[runs].y - read_only[name].y,
    editable[runs].y - editable[name].y,
    "and sits the same distance under the title"
  );
}
