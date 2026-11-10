//! An instruction's fields (A10 `hGdGu`, L5 `f8QbK3`; A27 `uVUZ4`): a new one's name, its format and *Text*, the
//! Markdown an agent reads. Who gets it, when and where it is added are not chosen in the form: a profile chooses the
//! instructions its agents are given (`docs/design/team-configuration.md`, "Decided 2026-11-09"), so a new one is
//! added to every run at the start, and an edit keeps what its version has.

use std::{collections::BTreeSet, sync::Arc};

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::resource::{AgentTargets, InjectionEvent, Instruction, InstructionScope, PromptText};
use orchester_workspace::{agents::form::prompt_error, instructions::publication};

use super::{Drawn, Form, Line, Look, Section};
use crate::{screens::team_hub::rows, ui::message::message_text};

const NAME: Line = Line {
  part: "name",
  label: "team_new.instruction.name",
  sub: Some("team_new.instruction.nameSub"),
  placeholder: Some("team_new.instruction.namePlaceholder"),
  look: Look::Plain,
  required: true,
};

/// The text is at least 180 high (`Input` `sTxOp` in A10 `hGdGu`).
const TEXT: Section = Section {
  part: "text",
  label: "team_hub.plain.text",
  placeholder: "team_new.instruction.textPlaceholder",
  height: 180.0,
};

/// The fields. Cloning shares them.
#[derive(Clone)]
pub(in crate::screens) struct InstructionFields {
  name: Signal<String>,
  text: Signal<String>,
}

impl InstructionFields {
  /// Empty fields, as a new instruction's.
  pub(in crate::screens) fn new(ctx: &mut Ctx) -> Self {
    Self {
      name: ctx.signal(String::new()),
      text: ctx.signal(String::new()),
    }
  }

  /// The name as typed.
  pub(in crate::screens) fn name(&self) -> String {
    self.name.get()
  }

  /// Every field takes the value `body` has.
  pub(in crate::screens) fn load(&self, body: &Instruction) {
    self.text.set(body.text().as_str().to_owned());
  }

  /// The form as `form` has it.
  pub(in crate::screens) fn draw(&self, ctx: &mut Ctx, form: &Form<'_, Instruction>) -> Drawn<Instruction> {
    let text = self.text.get();
    let made = made(ctx, &text, form.base);
    let name = form.name_row(ctx, &NAME, &self.name);
    let format = form.shown(
      "format",
      (ctx.t("team_hub.plain.format"), None),
      rows::words(&form.value_id("format"), &ctx.t("team_hub.plain.markdown")),
    );
    let error = form.said(made.as_ref().err().cloned());
    let was = form.base.map(|base| base.text().as_str());
    let section = form.section(ctx, &TEXT, &self.text, (was, error));
    Drawn {
      name,
      rows: vec![format],
      sections: vec![section],
      made,
    }
  }
}

/// The instruction the fields make: `text`, with who gets it, when and where it is added as `base` has them, or for a
/// new one every agent at the start of every run. A blank text, one that names a variable the catalog does not have,
/// or one whose variable has no value when it is added makes none, and says why.
fn made(ctx: &Ctx, text: &str, base: Option<&Instruction>) -> Result<Instruction, Arc<str>> {
  if text.trim().is_empty() {
    return Err(ctx.t("team_hub.edit.textRequired"));
  }
  let text = PromptText::new(text).map_err(|error| message_text(ctx, &prompt_error(&error)))?;
  let (targets, events, scope) = base.map_or_else(
    || {
      (
        AgentTargets::All,
        BTreeSet::from([InjectionEvent::Start]),
        InstructionScope::Project,
      )
    },
    |base| (base.targets().clone(), base.events().clone(), base.scope().clone()),
  );
  if !publication::missing(&text, &events).is_empty() {
    return Err(ctx.t("team_hub.edit.variablesMissing"));
  }
  Ok(
    Instruction::new(text, targets, events, scope)
      .expect("who gets it, when and where it is added are a version's own or every agent at the start of every run"),
  )
}
