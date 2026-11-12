//! The form of a new tool (`Agents · New tool` A12 `T4rWil`, L7 `q5aX9`): the tool's fields as its edit has them
//! ([`ToolFields`]: its name, kind and what a command runs), empty. A tool is the team's whole except the values of
//! the secrets it names, which each computer adds. A command is what this form makes; an HTTP request and a database
//! query are shown as not available, and Create stays off for them.

use lurq::app::ctx::Ctx;
use orchester_workspace::tools::form::tool_name;

use super::{
  fields::{self, Body, Shown, name_error},
  work::Make,
};
use crate::{
  screens::hub::forms::{Form, Ids, Naming, ToolFields},
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
    let drawn = self.fields.draw(
      ctx,
      &Form {
        ids: IDS,
        base: None,
        name: Some(Naming {
          said,
          first: Some(shown.first),
        }),
        tried: shown.tried,
        locked: shown.locked,
      },
    );
    // A kind this form cannot make says so at the bar's end, before the name does.
    let kind_off = drawn.made.is_err() && !self.fields.is_command();
    let make = match (named, drawn.made) {
      (Ok(name), Ok(tool)) => Some(Make::Tool(name, Box::new(tool))),
      _ => None,
    };
    Body {
      lead: None,
      rows: drawn.name.into_iter().chain(drawn.rows).collect(),
      after: drawn.sections,
      computer: Some("team_new.computer.tool"),
      make,
      off: if kind_off {
        Some("team_new.tool.kindOff")
      } else {
        fields::off(shown, &name)
      },
      overlay: None,
    }
  }
}
