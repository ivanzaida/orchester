//! The form of a new instruction (`Agents · New instruction` A10 `hGdGu`): its name, where it is added and its text. An
//! instruction is the team's whole: nothing is filled in on a computer afterwards.

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::name::Name;
use orchester_workspace::{
  instructions::form::{Events, InstructionForm, ScopeChoice, Target},
  messages::{self, Message},
};

use super::{
  fields::{self, Body, Error, Look, Shown, Spec, name_error},
  work::Make,
};
use crate::screens::team_hub::rows;

const NAME: Spec = Spec::new("instruction-name", "team_new.instruction.name")
  .sub("team_new.instruction.nameSub")
  .required();
const ADDED_TO: Spec =
  Spec::new("instruction-added-to", "team_new.instruction.addedTo").sub("team_new.instruction.addedToSub");
const TEXT: Spec = Spec::new("instruction-text", "team_new.instruction.text")
  .sub("team_new.instruction.textSub")
  .required();

pub(super) struct Fields {
  name: Signal<String>,
  text: Signal<String>,
}

impl Fields {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      name: ctx.signal(String::new()),
      text: ctx.signal(String::new()),
    }
  }

  pub(super) fn body(&self, ctx: &mut Ctx, shown: &Shown<'_>) -> Body {
    let (name, text) = (self.name.get(), self.text.get());
    let checked = check(&name, &text);
    let name_says = name_error(ctx, shown, &name, "team_new.instruction.nameTaken").or_else(|| {
      checked
        .name
        .clone()
        .filter(|_| shown.tried && !name.trim().is_empty())
        .map(Error::Said)
    });
    let text_says = shown
      .tried
      .then(|| match (&checked.text, text.trim().is_empty()) {
        (_, true) => Some(Error::Required),
        (Some(message), false) => Some(Error::Said(message.clone())),
        (None, false) => None,
      })
      .flatten();
    let rows = vec![
      fields::text_row(
        ctx,
        &NAME,
        name_says.as_ref(),
        (&self.name, "team_new.instruction.namePlaceholder"),
        (Look::Plain, shown.locked),
      ),
      fields::row(
        ctx,
        &ADDED_TO,
        None,
        rows::words("team-new-instruction-added-to", &ctx.t("team_new.instruction.everyRun")),
      ),
      fields::text_row(
        ctx,
        &TEXT,
        text_says.as_ref(),
        (&self.text, "team_new.instruction.textPlaceholder"),
        (Look::Area, shown.locked),
      ),
    ];
    Body {
      rows,
      computer: "team_new.computer.instruction",
      make: checked.make,
      off: fields::taken(shown, &name).map(|_| fields::FIX_NAME),
    }
  }
}

struct Checked {
  name: Option<Message>,
  text: Option<Message>,
  make: Option<Make>,
}

/// The instruction the fields make: added to every run, at the start of each.
fn check(name: &str, text: &str) -> Checked {
  let named = Name::new(name).map_err(|error| messages::name_error(&error));
  let body = if text.trim().is_empty() {
    None
  } else {
    Some(
      InstructionForm {
        text,
        targets: &[Target::All],
        events: Events::Start,
        scope: &ScopeChoice::Project,
      }
      .instruction(),
    )
  };
  let text_error = match &body {
    Some(Err(errors)) => errors.text.clone(),
    _ => None,
  };
  let make = match (named.as_ref(), body) {
    (Ok(named), Some(Ok(body))) => Some(Make::Instruction(named.clone(), Box::new(body))),
    _ => None,
  };
  Checked {
    name: named.err(),
    text: text_error,
    make,
  }
}
