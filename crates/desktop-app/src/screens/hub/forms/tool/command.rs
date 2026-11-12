//! A tool's fields (A12 `T4rWil`, L7 `q5aX9`; A29 `FrraS`): a new one's name, its kind, and what a command runs (its
//! description, program, arguments one per line and how long it may run). They are the command tool editor's
//! ([`CommandText`]), checked by the types they become. A new tool chooses its kind; an HTTP request and a database
//! query name secrets by name, which neither form can yet, so they are shown as not available and make nothing, and
//! only a command is edited. An edit keeps its exit codes, output limit, environment and access from its version.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_core::resource::Tool;
use orchester_workspace::tools::form::{CommandErrors, CommandText};

use super::{Drawn, Form, Line, Look};
use crate::{
  screens::team_hub::rows,
  ui::{
    message::message_text,
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
  },
};

const NAME: Line = Line {
  part: "name",
  label: "team_new.tool.name",
  sub: Some("team_new.tool.nameSub"),
  placeholder: Some("team_new.tool.namePlaceholder"),
  look: Look::Mono,
  required: true,
};
const DESCRIPTION: Line = Line {
  part: "description",
  label: "team_hub.plain.description",
  sub: None,
  placeholder: Some("team_new.tool.descriptionPlaceholder"),
  look: Look::Plain,
  required: false,
};
const PROGRAM: Line = Line {
  part: "program",
  label: "team_hub.tool.program",
  sub: None,
  placeholder: Some("team_new.tool.programPlaceholder"),
  look: Look::Mono,
  required: true,
};
/// The arguments' area is at least 64 high, a few lines, in Geist Mono as the program is.
const ARGUMENTS: Line = Line {
  part: "arguments",
  label: "team_hub.tool.arguments",
  sub: Some("team_hub.tool.argumentsSub"),
  placeholder: Some("team_hub.tool.argumentsPlaceholder"),
  look: Look::MonoArea(64.0),
  required: false,
};
const TIMEOUT: Line = Line {
  part: "timeout",
  label: "team_hub.tool.timeoutField",
  sub: None,
  placeholder: None,
  look: Look::Plain,
  required: false,
};

/// The kinds a new tool chooses from, by value and label; only a command is made.
const COMMAND: &str = "command";
const KINDS: [(&str, &str); 3] = [
  (COMMAND, "team_hub.tool.kind.command"),
  ("http", "team_hub.tool.kind.http"),
  ("database", "team_hub.tool.kind.database"),
];

/// The fields. Cloning shares them.
#[derive(Clone)]
pub(in crate::screens) struct ToolFields {
  name: Signal<String>,
  kind: Signal<String>,
  description: Signal<String>,
  program: Signal<String>,
  arguments: Signal<String>,
  timeout: Signal<String>,
}

impl ToolFields {
  /// A new command's fields: empty, but for how long it may run by default.
  pub(in crate::screens) fn new(ctx: &mut Ctx) -> Self {
    Self {
      name: ctx.signal(String::new()),
      kind: ctx.signal(COMMAND.to_owned()),
      description: ctx.signal(String::new()),
      program: ctx.signal(String::new()),
      arguments: ctx.signal(String::new()),
      timeout: ctx.signal(CommandText::new_tool().timeout),
    }
  }

  /// The name as typed.
  pub(in crate::screens) fn name(&self) -> String {
    self.name.get()
  }

  /// The kind chosen is a command, which is all the form makes.
  pub(in crate::screens) fn is_command(&self) -> bool {
    self.kind.get() == COMMAND
  }

  /// Every field takes `tool`'s value; a tool that is not a command leaves them as they are.
  pub(in crate::screens) fn load(&self, tool: &Tool) {
    let Some(text) = CommandText::of(tool) else {
      return;
    };
    self.description.set(text.description);
    self.program.set(text.program);
    self.arguments.set(text.arguments);
    self.timeout.set(text.timeout);
  }

  /// The form as `form` has it.
  pub(in crate::screens) fn draw(&self, ctx: &mut Ctx, form: &Form<'_, Tool>) -> Drawn<Tool> {
    let name = form.name_row(ctx, &NAME, &self.name);
    let kind = form.shown(
      "kind",
      (ctx.t("team_hub.tool.kindLabel"), Some(ctx.t("team_hub.tool.kindSub"))),
      self.kind_value(ctx, form),
    );
    let mut rows = vec![kind];
    if form.creates() && !self.is_command() {
      rows.push(form.shown(
        "unavailable",
        (ctx.t("team_new.tool.unavailable"), None),
        rows::words(&form.value_id("unavailable"), &ctx.t("team_new.tool.unavailableValue")),
      ));
      return Drawn {
        name,
        rows,
        sections: Vec::new(),
        made: Err(ctx.t("team_new.tool.kindOff")),
      };
    }
    let checked = self.checked(form.base);
    let errors = checked
      .as_ref()
      .err()
      .map(|errors| (**errors).clone())
      .unwrap_or_default();
    let said = |message: &Option<_>| form.said(message.as_ref().map(|message| message_text(ctx, message)));
    let (program, arguments, timeout) = (said(&errors.program), said(&errors.arguments), said(&errors.timeout));
    rows.extend([
      form.line(ctx, &DESCRIPTION, &self.description, (None, None)),
      form.line(ctx, &PROGRAM, &self.program, (program, None)),
      form.line(ctx, &ARGUMENTS, &self.arguments, (arguments, None)),
      form.line(ctx, &TIMEOUT, &self.timeout, (timeout, None)),
    ]);
    Drawn {
      name,
      rows,
      sections: Vec::new(),
      made: checked.map_err(|errors| waits(ctx, &errors)),
    }
  }

  /// A new tool's kind as a choice; an edited one's in words, a command.
  fn kind_value(&self, ctx: &Ctx, form: &Form<'_, Tool>) -> Element {
    let id = form.value_id("kind");
    if !form.creates() {
      return rows::words(&id, &ctx.t("team_hub.tool.kind.command")).into();
    }
    let chosen = self.kind.get();
    let locked = form.locked;
    let segments = KINDS.map(|(option, label)| {
      let kind = self.kind.clone();
      Segment::new(
        format!("{id}-{option}"),
        SegmentFace::Text(ctx.t(label)),
        chosen == option,
        move || {
          if !locked {
            kind.set(option.to_owned());
          }
        },
      )
    });
    segmented(&id, segments, Focus::Keyboard).into()
  }

  /// The tool the fields make, over `base`, which keeps its exit codes, output limit, environment and access, or as a
  /// new command; why it cannot be made, per field.
  fn checked(&self, base: Option<&Tool>) -> Result<Tool, Box<CommandErrors>> {
    let kept = base.and_then(CommandText::of).unwrap_or_else(CommandText::new_tool);
    CommandText {
      description: self.description.get().trim().to_owned(),
      program: self.program.get(),
      arguments: self.arguments.get(),
      timeout: self.timeout.get(),
      ..kept
    }
    .tool(base.and_then(|base| base.access().cloned()))
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
