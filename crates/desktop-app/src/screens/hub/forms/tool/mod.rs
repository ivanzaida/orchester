//! A tool's fields (A12 `T4rWil`, L7 `q5aX9`, A29 `FrraS` for a command; A32–A35, L20–L24 for an HTTP request and a
//! database query): a new one's name, its kind, its description, and what it runs or calls ([`command`],
//! [`request`]); an HTTP or database tool then has its *Access* ([`access`]), which it needs. A new tool chooses its
//! kind; an edit keeps it. A local project's tool is a [`Tool`], its secret picked from the project's; a team's is a
//! [`TeamTool`], its secret named, and each computer fills in its value.

mod access;
mod choice;
mod command;
mod request;

use std::sync::Arc;

use lurq::{app::ctx::Ctx, components::Row, core::Signal, node::Element};
use orchester_core::{
  resource::{TeamTool, Tool, ToolKind},
  secret::Secret,
};
use orchester_workspace::{
  agents::team_tool::CallingPipeline,
  tools::form::{CommandText, SecretToolErrors, SecretToolText},
};

use self::{
  access::{AccessFields, Team},
  choice::{chosen, segments, text_of},
  command::CommandFields,
  request::{RequestFields, Secrets},
};
use super::{Drawn, Form, Line, Look, Need};
use crate::screens::team_hub::rows;

const NAME: Line = Line {
  part: "name",
  label: "team_new.tool.name",
  sub: Some("team_new.tool.nameSub"),
  placeholder: Some("team_new.tool.namePlaceholder"),
  look: Look::Mono,
  required: Need::ToMake,
};
const DESCRIPTION: Line = Line {
  part: "description",
  label: "team_hub.plain.description",
  sub: None,
  placeholder: Some("team_new.tool.descriptionPlaceholder"),
  look: Look::Plain,
  required: Need::No,
};

/// The kinds a new tool chooses from, by value.
const KINDS: [(&str, ToolKind); 3] = [
  ("command", ToolKind::Command),
  ("http", ToolKind::Http),
  ("database", ToolKind::Database),
];

/// The words of a kind.
const fn kind_key(kind: ToolKind) -> &'static str {
  match kind {
    ToolKind::Command => "team_hub.tool.kind.command",
    ToolKind::Http => "team_hub.tool.kind.http",
    ToolKind::Database => "team_hub.tool.kind.database",
  }
}

/// The fields. Cloning shares them.
#[derive(Clone)]
pub(in crate::screens) struct ToolFields {
  name: Signal<String>,
  kind: Signal<String>,
  description: Signal<String>,
  command: CommandFields,
  request: RequestFields,
  access: AccessFields,
}

/// What an HTTP or database tool's fields draw, and the errors they say.
struct Called {
  rows: Vec<Row>,
  access: Element,
  marked: bool,
}

impl ToolFields {
  /// A new command's fields: empty, but for how long it may run and the access it would have by default.
  pub(in crate::screens) fn new(ctx: &mut Ctx) -> Self {
    Self {
      name: ctx.signal(String::new()),
      kind: ctx.signal(KINDS[0].0.to_owned()),
      description: ctx.signal(String::new()),
      command: CommandFields::new(ctx),
      request: RequestFields::new(ctx),
      access: AccessFields::new(ctx),
    }
  }

  /// The name as typed.
  pub(in crate::screens) fn name(&self) -> String {
    self.name.get()
  }

  /// The kind chosen, or an edited tool's.
  pub(in crate::screens) fn kind(&self) -> ToolKind {
    chosen(&self.kind, &KINDS)
  }

  /// What each computer fills in once the team's tool is made: the value of the secret it names, in the environment it
  /// uses (`Computer` note in A32 `gLLEv`), once both are given.
  pub(in crate::screens) fn fills_in(&self, ctx: &Ctx) -> Arc<str> {
    let kind = self.kind();
    let named = self.request.named();
    let environment = self.access.environment();
    match environment {
      Some(environment)
        if kind != ToolKind::Command && self.secret_text(kind).takes_secret() && !named.trim().is_empty() =>
      {
        ctx.t_args(
          "team_new.computer.toolSecret",
          [
            ("secret", named.trim()),
            ("environment", &*ctx.t(access::environment_key(environment))),
          ],
        )
      }
      _ => ctx.t("team_new.computer.tool"),
    }
  }

  /// Every field takes the local `tool`'s value.
  pub(in crate::screens) fn load(&self, tool: &Tool) {
    self.kind.set(text_of(tool.kind(), &KINDS).to_owned());
    self.description.set(tool.description().to_owned());
    if let Some(text) = CommandText::of(tool) {
      self.command.load(text);
    }
    if let Some((text, picked)) = SecretToolText::of(tool) {
      self.request.load(text.action, (None, picked));
      self.access.load(text.access);
    }
  }

  /// Every field takes the team's `tool`'s value.
  pub(in crate::screens) fn load_team(&self, tool: &TeamTool) {
    self.kind.set(text_of(tool.kind(), &KINDS).to_owned());
    self.description.set(tool.description().to_owned());
    if let Some(text) = tool.command_tool().as_ref().and_then(CommandText::of) {
      self.command.load(text);
    }
    if let Some((text, named)) = SecretToolText::of_team(tool) {
      self
        .request
        .load(text.action, (named.map(|name| name.to_string()), None));
      self.access.load(text.access);
    }
  }

  /// The form of a local project's tool as `form` has it, its secret picked from `secrets`, the project's that are not
  /// archived (`None` while they are read).
  pub(in crate::screens) fn draw(
    &self,
    ctx: &mut Ctx,
    form: &Form<'_, Tool>,
    secrets: Option<&[Secret]>,
  ) -> Drawn<Tool> {
    let (name, mut rows) = self.head(ctx, form);
    let kind = self.kind();
    if kind == ToolKind::Command {
      let kept = form
        .base
        .and_then(|base| Some((CommandText::of(base)?, base.access().cloned())));
      let (more, made) = self.command.draw(ctx, form, (self.description(), kept));
      rows.extend(more);
      return command_drawn(name, rows, made);
    }
    let environment = self.access.environment();
    let text = self.secret_text(kind);
    let checked = text.tool(self.request.picked(secrets, environment));
    let errors = errors_of(checked.as_ref().err().map(|errors| &**errors));
    let picked = Secrets::Picked { secrets, environment };
    let called = self.called(ctx, form, (kind, &errors, picked), None);
    self.drawn(ctx, (name, rows), called, checked)
  }

  /// The form of a team's tool as `form` has it; `pipelines` are the team's, which may call it.
  pub(in crate::screens) fn draw_team(
    &self,
    ctx: &mut Ctx,
    form: &Form<'_, TeamTool>,
    pipelines: &[CallingPipeline],
  ) -> Drawn<TeamTool> {
    let (name, mut rows) = self.head(ctx, form);
    let kind = self.kind();
    if kind == ToolKind::Command {
      let kept = form.base.and_then(|base| {
        let local = base.command_tool()?;
        Some((CommandText::of(&local)?, base.access().cloned()))
      });
      let (more, made) = self.command.draw(ctx, form, (self.description(), kept));
      rows.extend(more);
      return command_drawn(name, rows, made.map(|tool| team_command(&tool)));
    }
    let text = self.secret_text(kind);
    let checked = text.team_tool(&self.request.named());
    let errors = errors_of(checked.as_ref().err().map(|errors| &**errors));
    let base = form.base.and_then(TeamTool::access);
    let called = self.called(
      ctx,
      form,
      (kind, &errors, Secrets::Named),
      Some(Team { pipelines, base }),
    );
    self.drawn(ctx, (name, rows), called, checked)
  }

  /// The name row of a new tool, its kind and its description.
  fn head<B>(&self, ctx: &mut Ctx, form: &Form<'_, B>) -> (Option<Row>, Vec<Row>) {
    let name = form.name_row(ctx, &NAME, &self.name);
    let kind = form.shown(
      "kind",
      (ctx.t("team_hub.tool.kindLabel"), Some(ctx.t("team_hub.tool.kindSub"))),
      self.kind_value(ctx, form),
    );
    let description = form.line(ctx, &DESCRIPTION, &self.description, (None, None));
    (name, vec![kind, description])
  }

  /// A new tool's kind as a choice; an edited one's in words.
  fn kind_value<B>(&self, ctx: &Ctx, form: &Form<'_, B>) -> Element {
    let id = form.value_id("kind");
    if !form.creates() {
      return rows::words(&id, &ctx.t(kind_key(self.kind()))).into();
    }
    let options = KINDS.map(|(value, kind)| (value, ctx.t(kind_key(kind))));
    segments(&id, &self.kind, options, form.locked)
  }

  fn description(&self) -> String {
    self.description.get().trim().to_owned()
  }

  fn secret_text(&self, kind: ToolKind) -> SecretToolText {
    SecretToolText {
      description: self.description(),
      action: self.request.action(kind),
      access: self.access.text(),
    }
  }

  /// The rows of what an HTTP or database tool calls and its *Access*; `team` has the team's pipelines and the access
  /// an edit starts from.
  fn called<B>(
    &self,
    ctx: &mut Ctx,
    form: &Form<'_, B>,
    (kind, errors, secrets): (ToolKind, &SecretToolErrors, Secrets<'_>),
    team: Option<Team<'_>>,
  ) -> Called {
    let (rows, marked) = self.request.rows(ctx, form, (kind, errors, secrets));
    let access = self.access.section(ctx, form, (&errors.access, team));
    Called { rows, access, marked }
  }

  /// What the form draws of an HTTP or database tool, which `checked` made or not.
  fn drawn<B, E>(
    &self,
    ctx: &Ctx,
    (name, mut rows): (Option<Row>, Vec<Row>),
    called: Called,
    checked: Result<B, E>,
  ) -> Drawn<B> {
    rows.extend(called.rows);
    Drawn {
      name,
      rows,
      sections: vec![called.access],
      made: checked.map_err(|_| ctx.t("team_hub.edit.fixFields")),
      marked: called.marked,
    }
  }
}

/// What the form draws of a command.
fn command_drawn<B>(name: Option<Row>, rows: Vec<Row>, made: Result<B, Arc<str>>) -> Drawn<B> {
  Drawn {
    name,
    rows,
    sections: Vec::new(),
    made,
    marked: false,
  }
}

/// A command tool in the team's form, which is the local one's: a command names no secret.
fn team_command(tool: &Tool) -> TeamTool {
  TeamTool::from_local(tool, |_| None).expect("a command tool names no secret and is a team tool as it is")
}

fn errors_of(errors: Option<&SecretToolErrors>) -> SecretToolErrors {
  errors.cloned().unwrap_or_default()
}
