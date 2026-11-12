//! What a command tool runs (A12 `T4rWil`, L7 `q5aX9`, A29 `FrraS`): its program, arguments one per line and how long
//! it may run. They are the command tool editor's ([`CommandText`]), checked by the types they become. An edit keeps
//! its exit codes, output limit, environment and access from its version.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, components::Row, core::Signal};
use orchester_core::resource::{AccessPolicy, Tool};
use orchester_workspace::tools::form::{CommandErrors, CommandText};

use super::super::{Form, Line, Look, Need};
use crate::ui::message::message_text;

const PROGRAM: Line = Line {
  part: "program",
  label: "team_hub.tool.program",
  sub: None,
  placeholder: Some("team_new.tool.programPlaceholder"),
  look: Look::Mono,
  required: Need::ToMake,
};
/// The arguments' area is at least 64 high, a few lines, in Geist Mono as the program is.
const ARGUMENTS: Line = Line {
  part: "arguments",
  label: "team_hub.tool.arguments",
  sub: Some("team_hub.tool.argumentsSub"),
  placeholder: Some("team_hub.tool.argumentsPlaceholder"),
  look: Look::MonoArea(64.0),
  required: Need::No,
};
const TIMEOUT: Line = Line {
  part: "timeout",
  label: "team_hub.tool.timeoutField",
  sub: None,
  placeholder: None,
  look: Look::Plain,
  required: Need::No,
};

/// The fields. Cloning shares them.
#[derive(Clone)]
pub(super) struct CommandFields {
  program: Signal<String>,
  arguments: Signal<String>,
  timeout: Signal<String>,
}

impl CommandFields {
  /// A new command's fields: empty, but for how long it may run by default.
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      program: ctx.signal(String::new()),
      arguments: ctx.signal(String::new()),
      timeout: ctx.signal(CommandText::new_tool().timeout),
    }
  }

  /// Every field takes the value `text` has.
  pub(super) fn load(&self, text: CommandText) {
    self.program.set(text.program);
    self.arguments.set(text.arguments);
    self.timeout.set(text.timeout);
  }

  /// The rows, and the tool they make with `description` over `kept`, the version's command and access, which keeps
  /// what the form does not hold; or as a new command.
  pub(super) fn draw<B>(
    &self,
    ctx: &mut Ctx,
    form: &Form<'_, B>,
    (description, kept): (String, Option<(CommandText, Option<AccessPolicy>)>),
  ) -> (Vec<Row>, Result<Tool, Arc<str>>) {
    let (kept, access) = kept.unwrap_or_else(|| (CommandText::new_tool(), None));
    let checked = CommandText {
      description,
      program: self.program.get(),
      arguments: self.arguments.get(),
      timeout: self.timeout.get(),
      ..kept
    }
    .tool(access);
    let errors = checked
      .as_ref()
      .err()
      .map(|errors| (**errors).clone())
      .unwrap_or_default();
    let said = |message: &Option<_>| form.said(message.as_ref().map(|message| message_text(ctx, message)));
    let (program, arguments, timeout) = (said(&errors.program), said(&errors.arguments), said(&errors.timeout));
    let rows = vec![
      form.line(ctx, &PROGRAM, &self.program, (program, None)),
      form.line(ctx, &ARGUMENTS, &self.arguments, (arguments, None)),
      form.line(ctx, &TIMEOUT, &self.timeout, (timeout, None)),
    ];
    (rows, checked.map_err(|errors| waits(ctx, &errors)))
  }
}

/// Why the fields make no tool, in the words of the first thing wrong.
fn waits(ctx: &Ctx, errors: &CommandErrors) -> Arc<str> {
  [
    &errors.program,
    &errors.arguments,
    &errors.timeout,
    &errors.exit_codes,
    &errors.output_limit,
    &errors.environment,
  ]
  .into_iter()
  .find_map(|error| error.as_ref())
  .map_or_else(|| ctx.t("team_hub.edit.fixFields"), |error| message_text(ctx, error))
}
