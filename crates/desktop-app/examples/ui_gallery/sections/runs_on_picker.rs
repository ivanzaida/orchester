//! The compact *Runs on* picker (`Runs on picker` `ZcGGy` on `Pipelines · v2` `pCzIS`; K1 `lCApz`, K2 `rjPcV`): open
//! over fixture connections, applying each choice to its own chosen state as a screen would. Type to see K2
//! (searching).

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_app::ui::{
  agent_mark::AgentMark,
  runs_on_picker::{Chosen, Connection, Mark, Model, Pick, RunsOnPicker},
};

use super::cell;
use crate::gallery::State;

fn model(id: &str, efforts: &[&str]) -> Model {
  Model {
    id: id.into(),
    efforts: efforts.iter().map(|effort| Arc::from(*effort)).collect(),
  }
}

fn connection(id: &str, name: &str, mark: Mark, models: Vec<Model>) -> Connection {
  Connection {
    id: id.into(),
    mark,
    name: name.into(),
    caption: "Claude Code · Max plan".into(),
    tip: format!("Claude Code · {name}").into(),
    five_hour: Some(41),
    limit: false,
    unavailable: None,
    models,
  }
}

/// Fixture connections, as K1 draws them: two of Claude Code (the second at its limit), two of Codex (the first signed
/// out), and an API provider.
fn connections() -> Vec<Connection> {
  let efforts = ["low", "medium", "high", "xhigh", "max"];
  let claude_models = || {
    vec![
      model("claude-opus-5-5", &efforts),
      model("claude-sonnet-5-5", &efforts),
      model("claude-fable-5-1", &efforts),
      model("claude-haiku-4-5-20251001", &[]),
      model("opus", &efforts),
      model("sonnet", &efforts),
    ]
  };
  let (claude, codex) = (Mark::Agent(AgentMark::Claude), Mark::Agent(AgentMark::OpenAi));
  let codex_models = || vec![model("gpt-5.5", &["low", "medium", "high"])];
  vec![
    connection("work", "Work", claude, claude_models()),
    Connection {
      limit: true,
      five_hour: Some(100),
      ..connection("home", "Home", claude, claude_models())
    },
    Connection {
      unavailable: Some("Codex · Default is signed out".into()),
      caption: "Codex · Plus plan".into(),
      ..connection("default", "Default", codex, codex_models())
    },
    Connection {
      caption: "Codex · Pro plan".into(),
      ..connection("team", "Team", codex, codex_models())
    },
    Connection {
      five_hour: None,
      caption: "Claude Code · API · api.deepseek.com".into(),
      ..connection("deepseek", "DeepSeek", Mark::Cloud, vec![model("deepseek-v4-pro", &[])])
    },
  ]
}

pub fn section(ctx: &mut Ctx, state: &State) -> Element {
  let chosen = state.runs_on.clone();
  let picker = RunsOnPicker::new(
    "g-runs-on",
    (connections(), state.runs_on.get()),
    move |pick| chosen.update(|chosen| apply(chosen, pick)),
    || {},
  );
  cell("Runs on picker · ZcGGy (K1 · type for K2)", picker.element(ctx)).into()
}

/// What the picker's caller does with a choice: runs on it, with the connection's first model when it is chosen.
fn apply(chosen: &mut Chosen, pick: Pick) {
  match pick {
    Pick::Connection(id) => {
      let first = connections()
        .into_iter()
        .find(|connection| connection.id == id)
        .and_then(|connection| connection.models.first().map(|first| first.id.clone()));
      *chosen = Chosen {
        connection: id,
        model: first,
        effort: None,
      };
    }
    Pick::Model { connection, model } => {
      let offered = connections()
        .into_iter()
        .find(|candidate| candidate.id == connection)
        .and_then(|candidate| candidate.models.into_iter().find(|candidate| candidate.id == model))
        .is_some_and(|candidate| {
          chosen
            .effort
            .as_ref()
            .is_some_and(|effort| candidate.efforts.contains(effort))
        });
      chosen.connection = connection;
      chosen.model = Some(model);
      if !offered {
        chosen.effort = None;
      }
    }
    Pick::Effort(effort) => chosen.effort = effort,
  }
}

/// What the gallery's picker starts with chosen (K1: Work · claude-opus-5-5 · High).
pub fn start() -> Chosen {
  Chosen {
    connection: "work".into(),
    model: Some("claude-opus-5-5".into()),
    effort: Some("high".into()),
  }
}
