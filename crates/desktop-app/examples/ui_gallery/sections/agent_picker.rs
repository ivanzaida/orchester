//! The Agent picker (`Agent picker` `Zn8BM` in K4 `CwLfL`): open over the fixture agents of K4, applying each choice
//! to its own chosen state as a screen would. Type to filter; ↑ ↓ move the keys, Enter chooses. A second picker lists
//! sixteen agents, so that its list scrolls.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_app::ui::{
  agent_mark::AgentMark,
  agent_picker::{Actions, AgentPicker, Group, Item, Mark},
  icon::Icon,
};

use super::{cell, line};
use crate::gallery::State;

fn profile(key: &str, name: &str, runs_on: &str, mark: AgentMark) -> Item {
  Item {
    key: key.into(),
    group: Group::Profiles,
    mark: Mark::Agent(mark),
    name: name.into(),
    runs_on: runs_on.into(),
    suffix: None,
    warning: None,
  }
}

/// K4's list: four profiles of Claude Code (the third not ready), one of Codex, and a tool.
fn items() -> Vec<Item> {
  let claude = |key, name, runs_on| profile(key, name, runs_on, AgentMark::Claude);
  vec![
    claude("builder", "Builder", "Claude Code · Work · claude-opus-5-5"),
    claude("planner", "Planner", "Claude Code · Work · claude-opus-5-5 · read-only"),
    Item {
      warning: Some("Choose a Claude Code connection".into()),
      ..claude("explorer", "Explorer", "claude-opus-5-5")
    },
    profile(
      "notes",
      "Release notes",
      "Codex · Codex main · gpt-6.1-sol",
      AgentMark::OpenAi,
    ),
    Item {
      key: "checks".into(),
      group: Group::Tools,
      mark: Mark::Icon(Icon::Terminal),
      name: "Checks".into(),
      runs_on: "npm test".into(),
      suffix: Some("· its exit code decides the step".into()),
      warning: None,
    },
  ]
}

/// Sixteen profiles, more than the list has room for.
fn many() -> Vec<Item> {
  (1..=16)
    .map(|number| {
      profile(
        &format!("p{number}"),
        &format!("Profile {number}"),
        "Codex · Codex main · gpt-6.1-sol",
        AgentMark::OpenAi,
      )
    })
    .collect()
}

pub fn section(ctx: &mut Ctx, state: &State) -> Element {
  let chosen = state.agent.clone();
  let picker = AgentPicker::new(
    "g-agent",
    (items(), Some(state.agent.get().into())),
    Actions {
      on_pick: Box::new(move |key| chosen.set(key.to_string())),
      on_close: Box::new(|| {}),
      on_new: Box::new(|| {}),
      on_agents: Box::new(|| {}),
    },
  );
  let scrolling = AgentPicker::new(
    "g-agent-many",
    (many(), Some("p12".into())),
    Actions {
      on_pick: Box::new(|_| {}),
      on_close: Box::new(|| {}),
      on_new: Box::new(|| {}),
      on_agents: Box::new(|| {}),
    },
  );
  line([
    cell("Agent picker · Zn8BM (K4 · type to filter)", picker.element(ctx)),
    cell("Sixteen profiles · the list scrolls", scrolling.element(ctx)),
  ])
  .into()
}

/// What the gallery's picker starts with chosen (K4: Builder).
pub fn start() -> String {
  "builder".into()
}
