//! The form of a new tool (`Agents · New tool` A12 `T4rWil`): its name, what kind it is and what it runs. A tool is the
//! team's whole except the values of the secrets it names, which each computer adds. A command is what this form
//! makes; an HTTP request and a database query name secrets by name, which a new tool of this computer cannot yet, so
//! they are shown as not available and Create stays off for them.

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_workspace::tools::form::{CommandErrors, CommandText, tool_name};

use super::{
  fields::{self, Body, Error, Look, Shown, Spec, name_error},
  work::Make,
};
use crate::screens::team_hub::rows;

const NAME: Spec = Spec::new("tool-name", "team_new.tool.name")
  .sub("team_new.tool.nameSub")
  .required();
const KIND: Spec = Spec::new("tool-kind", "team_new.tool.kind");
const PROGRAM: Spec = Spec::new("tool-program", "team_new.tool.program")
  .sub("team_new.tool.programSub")
  .required();
const ARGUMENTS: Spec = Spec::new("tool-arguments", "team_new.tool.arguments").sub("team_new.tool.argumentsSub");
const DESCRIPTION: Spec = Spec::new("tool-description", "team_new.tool.description");

const COMMAND: &str = "command";
const KINDS: [(&str, &str); 3] = [
  (COMMAND, "team_new.tool.command"),
  ("http", "team_new.tool.http"),
  ("database", "team_new.tool.database"),
];

pub(super) struct Fields {
  name: Signal<String>,
  kind: Signal<String>,
  program: Signal<String>,
  arguments: Signal<String>,
  description: Signal<String>,
}

impl Fields {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      name: ctx.signal(String::new()),
      kind: ctx.signal(COMMAND.to_owned()),
      program: ctx.signal(String::new()),
      arguments: ctx.signal(String::new()),
      description: ctx.signal(String::new()),
    }
  }

  pub(super) fn body(&self, ctx: &mut Ctx, shown: &Shown<'_>) -> Body {
    let name = self.name.get();
    let command = self.kind.get() == COMMAND;
    let name_says = name_error(
      ctx,
      shown,
      &name,
      shown.mode.words("team_new.tool.nameTaken", "local_new.nameTaken.tool"),
    );
    let mut rows = vec![
      fields::name_row(
        ctx,
        shown,
        &NAME,
        name_says.as_ref(),
        (&self.name, "team_new.tool.namePlaceholder"),
        Look::Mono,
      ),
      fields::row(
        ctx,
        &KIND,
        None,
        fields::choice(ctx, &KIND, &self.kind, &KINDS, shown.locked),
      ),
    ];
    let mut make = None;
    if command {
      let (command_rows, made) = self.command(ctx, shown, &name);
      rows.extend(command_rows);
      make = made;
    } else {
      rows.push(fields::row(
        ctx,
        &Spec::new("tool-unavailable", "team_new.tool.unavailable"),
        None,
        rows::words(
          "team-new-tool-unavailable-value",
          &ctx.t("team_new.tool.unavailableValue"),
        ),
      ));
    }
    let off = if command {
      fields::off(shown, &name)
    } else {
      Some("team_new.tool.kindOff")
    };
    Body {
      lead: None,
      rows,
      computer: Some("team_new.computer.tool"),
      make,
      off,
      overlay: None,
    }
  }

  /// A command's rows, and the tool they make when they are valid.
  fn command(&self, ctx: &mut Ctx, shown: &Shown<'_>, name: &str) -> (Vec<lurq::components::Row>, Option<Make>) {
    let text = CommandText {
      description: self.description.get().trim().to_owned(),
      program: self.program.get(),
      arguments: self.arguments.get(),
      ..CommandText::new_tool()
    };
    let named = tool_name(name);
    let checked = text.tool(None);
    let errors: CommandErrors = match &checked {
      Err(errors) => (**errors).clone(),
      Ok(_) => CommandErrors::default(),
    };
    let program_says = if shown.tried && self.program.get().trim().is_empty() {
      Some(Error::Required)
    } else {
      errors
        .program
        .clone()
        .filter(|_| shown.tried)
        .or_else(|| errors.arguments.clone().filter(|_| shown.tried))
        .map(Error::Said)
    };
    let rows = vec![
      fields::text_row(
        ctx,
        &PROGRAM,
        program_says.as_ref(),
        (&self.program, "team_new.tool.programPlaceholder"),
        (Look::Mono, shown.locked),
      ),
      fields::text_row(
        ctx,
        &ARGUMENTS,
        None,
        (&self.arguments, "team_new.tool.argumentsPlaceholder"),
        (Look::Lines, shown.locked),
      ),
      fields::text_row(
        ctx,
        &DESCRIPTION,
        None,
        (&self.description, "team_new.tool.descriptionPlaceholder"),
        (Look::Plain, shown.locked),
      ),
    ];
    let make = match (named, checked) {
      (Ok(name), Ok(tool)) => Some(Make::Tool(name, Box::new(tool))),
      _ => None,
    };
    (rows, make)
  }
}
