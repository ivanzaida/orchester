//! The form of a new tool (`Agents · New tool` A12 `T4rWil`, L7 `q5aX9`; A32 `gLLEv`, A34 `WUuEj`, L20 `Kb6Vk`, L23
//! `cZGTp`): the tool's fields as its edit has them ([`ToolFields`]: its name, kind, and what it runs or calls, with an
//! HTTP request's or a database query's *Access*), empty. A team's tool is the team's whole except the values of the
//! secrets it names, which each computer adds; a local one picks its secret from the project's.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, components::Row, node::Element};
use orchester_core::name::Name;
use orchester_workspace::tools::form::tool_name;

use super::{
  fields::{self, Body, Shown, name_error},
  mode::Mode,
  work::Make,
};
use crate::{
  screens::hub::forms::{Drawn, Form, Ids, LocalSecrets, Naming, ToolFields},
  ui::message::message_text,
};

/// The ids of the form's parts, which tests and automation address: `team-new-tool-name-input`,
/// `team-new-tool-program-input`, `team-new-tool-kind-value-http`, …
const IDS: Ids = Ids {
  rows: "team-new-tool",
  sections: "team-new-tool",
};

pub(super) struct Fields {
  fields: ToolFields,
}

impl Fields {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      fields: ToolFields::new(ctx),
    }
  }

  pub(super) fn body(&self, ctx: &mut Ctx, shown: &Shown<'_>) -> Body {
    let name = self.fields.name();
    let named = tool_name(&name);
    let taken_key = shown.mode.words("team_new.tool.nameTaken", "local_new.nameTaken.tool");
    let said = name_error(ctx, shown, &name, taken_key)
      .map(|error| error.words(ctx))
      .or_else(|| {
        named
          .as_ref()
          .err()
          .filter(|_| shown.tried && !name.trim().is_empty())
          .map(|message| message_text(ctx, message))
      });
    let named = named.ok();
    let made = match shown.mode {
      Mode::Team => Made::of(
        self.fields.draw_team(ctx, &form(said, shown), shown.pipelines),
        named,
        Make::TeamTool,
      ),
      Mode::Local => {
        let local = LocalSecrets {
          listed: shown.connections.map(|connections| &connections.secrets[..]),
          valueless: &[],
        };
        Made::of(self.fields.draw(ctx, &form(said, shown), local), named, Make::Tool)
      }
    };
    Body {
      lead: None,
      rows: made.rows,
      after: made.sections,
      computer: Some(self.fields.fills_in(ctx)),
      make: made.make,
      // What was typed in a field that cannot be used says so at the bar's end, before the name does (L22 `Xp89k`).
      off: if made.marked {
        Some("team_new.bar.fixFields")
      } else {
        fields::off(shown, &name)
      },
      overlay: None,
    }
  }
}

/// The form as `shown` has it, its name saying `said`.
fn form<'a, B>(said: Option<Arc<str>>, shown: &Shown<'a>) -> Form<'a, B> {
  Form {
    ids: IDS,
    base: None,
    name: Some(Naming {
      said,
      first: Some(shown.first),
    }),
    tried: shown.tried,
    locked: shown.locked,
  }
}

/// What the form draws, and what Create makes of it, once the name and every field are valid.
struct Made {
  rows: Vec<Row>,
  sections: Vec<Element>,
  marked: bool,
  make: Option<Make>,
}

impl Made {
  fn of<B>(drawn: Drawn<B>, name: Option<Name>, make: fn(Name, Box<B>) -> Make) -> Self {
    Self {
      rows: drawn.name.into_iter().chain(drawn.rows).collect(),
      sections: drawn.sections,
      marked: drawn.marked,
      make: name.zip(drawn.made.ok()).map(|(name, body)| make(name, Box::new(body))),
    }
  }
}
