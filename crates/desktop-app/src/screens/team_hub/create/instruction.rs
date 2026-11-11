//! The form of a new instruction (`Agents · New instruction` A10 `hGdGu`, L5 `f8QbK3`): the instruction's fields as
//! its edit has them ([`InstructionFields`]: its name, format and text), empty, with who gets it after its name.
//! An instruction is the team's whole: nothing is filled in on a computer afterwards. Nothing chooses where it is
//! added or who gets it, in a team's project or a local one: a profile chooses the instructions its agents are given
//! (`docs/design/team-configuration.md`, "Decided 2026-11-09"), so the form says that instead.

use lurq::app::ctx::Ctx;
use orchester_core::name::Name;
use orchester_workspace::messages;

use super::{
  fields::{self, Body, Error, Shown, name_error},
  work::Make,
};
use crate::{
  screens::{
    hub::forms::{Form, Ids, InstructionFields, Naming},
    team_hub::rows,
  },
  ui::message::message_text,
};

/// The ids of the form's parts, which tests and automation address: `team-new-instruction-name-input`,
/// `team-new-instruction-text-input`, …
const IDS: Ids = Ids {
  rows: "team-new-instruction",
  sections: "team-new-instruction",
};

pub(super) struct Fields {
  fields: InstructionFields,
}

impl Fields {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      fields: InstructionFields::new(ctx),
    }
  }

  pub(super) fn body(&self, ctx: &mut Ctx, shown: &Shown<'_>) -> Body {
    let name = self.fields.name();
    let named = Name::new(&name).map_err(|error| messages::name_error(&error));
    let taken_key = shown
      .mode
      .words("team_new.instruction.nameTaken", "local_new.nameTaken.instruction");
    let said = name_error(ctx, shown, &name, taken_key)
      .or_else(|| {
        named
          .as_ref()
          .err()
          .filter(|_| shown.tried && !name.trim().is_empty())
          .map(|message| Error::Words(message_text(ctx, message)))
      })
      .map(|error| error.words(ctx));
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
    // A profile, the team's or a local one, chooses the instructions its agents are given (*+ Add* in its panel).
    let added = rows::row(
      "team-new-instruction-given-to",
      &ctx.t("local_new.instruction.givenTo"),
      Some(rows::Sub::quiet(ctx.t("local_new.instruction.givenToSub"))),
      rows::words(
        "team-new-instruction-given-to-value",
        &ctx.t("local_new.instruction.givenToValue"),
      ),
    );
    let make = match (named, drawn.made) {
      (Ok(named), Ok(body)) => Some(Make::Instruction(named, Box::new(body))),
      _ => None,
    };
    Body {
      lead: None,
      rows: drawn.name.into_iter().chain([added]).chain(drawn.rows).collect(),
      after: drawn.sections,
      computer: Some("team_new.computer.instruction"),
      make,
      off: fields::off(shown, &name),
      overlay: None,
    }
  }
}
