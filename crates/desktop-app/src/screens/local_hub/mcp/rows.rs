//! *Server* of a local MCP server's panel (`Server` in L2, L8): what it runs as, its program with *Choose…*, its
//! arguments, folder, the tools that run without asking, its variables and the secret values they take, and the switch
//! that turns it on. A row says under its name when the field was changed and not saved yet.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, SelectOption, Text},
  layout::Alignment,
  node::dimension::Dimension,
};
use orchester_core::sharing::{LocalProgramProblem, Missing};
use orchester_workspace::connections::{
  data::ProjectConnections,
  mcp::{McpServerErrors, McpServerText, TransportChoice},
};

use super::{
  editor::Editor,
  form::Form,
  secrets::{self, Mark, Taken},
};
use crate::{
  screens::team_hub::rows::{self, Sub},
  theme::HubText,
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, SelectField},
    icon::Icon,
    message::message_text,
    toggle::{Toggle, ToggleKind},
  },
};

/// What a render of the rows is told about the panel.
pub(in crate::screens) struct Look<'a> {
  pub saved: &'a McpServerText,
  pub typed: &'a McpServerText,
  pub listed: &'a ProjectConnections,
  pub errors: &'a McpServerErrors,
  pub tried: bool,
  /// How the program stands as saved.
  pub program: Option<&'a [Missing]>,
  /// The secrets the variables take, and what the last save did to them.
  pub secrets: (&'a [Taken], Mark),
  /// The save was refused for the program.
  pub program_refused: bool,
  /// The server is being made: nothing is saved yet to differ from, and the program is required.
  pub creating: bool,
}

impl Look<'_> {
  /// *changed, not saved yet* under a field whose text is not what was saved; a server being made has nothing saved.
  fn changed(&self, ctx: &Ctx, now: &str, before: &str) -> Option<Sub> {
    (!self.creating && now != before).then(|| Sub {
      words: ctx.t("local_hub.mcp.changedSub"),
      color: PaletteColor::Warning,
    })
  }
}

impl Form {
  /// The rows of *Server*, the secret rows and the switch, in one outline.
  pub(in crate::screens) fn server_rows(&self, ctx: &mut Ctx, look: &Look<'_>) -> Vec<Row> {
    let mut rows = vec![self.runs_as(ctx)];
    match self.fields.transport.get() {
      TransportChoice::Stdio => {
        rows.extend(self.program_rows(ctx, look));
        rows.push(self.tools_row(ctx, look));
        rows.push(self.variables_row(ctx, look));
      }
      TransportChoice::Http => {
        rows.extend(self.remote_rows(ctx, look));
        rows.push(self.tools_row(ctx, look));
      }
    }
    rows
  }

  /// *Runs as*: a program on this computer, or a remote server.
  fn runs_as(&self, ctx: &Ctx) -> Row {
    let select = SelectField::new(
      "local-hub-mcp-runs-as",
      ctx.t("local_hub.mcp.runsAs"),
      self.fields.transport.clone(),
      vec![
        SelectOption::new(TransportChoice::Stdio, ctx.t("local_hub.mcp.asProgram")),
        SelectOption::new(TransportChoice::Http, ctx.t("local_hub.mcp.asRemote")),
      ],
    )
    .render(ctx);
    rows::row(
      "local-hub-mcp-row-runs-as",
      &ctx.t("local_hub.mcp.runsAs"),
      None,
      select,
    )
  }

  /// The program and its *Choose…*, the arguments and the folder of a program.
  fn program_rows(&self, ctx: &mut Ctx, look: &Look<'_>) -> Vec<Row> {
    let program = self.program_row(ctx, look);
    let arguments = self.arguments_row(ctx, look);
    let folder = self.folder_row(ctx, look);
    vec![program, arguments, folder]
  }

  /// *Program*: its path, whether it is found, and *Choose…*.
  fn program_row(&self, ctx: &mut Ctx, look: &Look<'_>) -> Row {
    let found = look.program.is_some_and(|missing| {
      !missing
        .iter()
        .any(|part| matches!(part, Missing::LocalMcpProgram { .. }))
    });
    let moved = look.program.is_some_and(|missing| {
      missing.iter().any(|part| {
        matches!(
          part,
          Missing::LocalMcpProgram {
            problem: LocalProgramProblem::Moved,
            ..
          }
        )
      })
    });
    let program_sub = if look.creating {
      Some(Sub::quiet(ctx.t("local_hub.mcp.programSub")))
    } else if look.typed.program != look.saved.program {
      look.changed(ctx, &look.typed.program, &look.saved.program)
    } else if found {
      Some(Sub::quiet(ctx.t("local_hub.mcp.found")))
    } else {
      let words = if moved {
        ctx.t("local_hub.mcp.moved")
      } else {
        ctx.t_args("local_hub.mcp.pathTo", [("program", look.saved.program.as_str())])
      };
      Some(Sub {
        words,
        color: PaletteColor::Warning,
      })
    };
    let mut input = FieldInput::new(
      "connection-mcp-program",
      ctx.t("connections.mcp.property.program"),
      self.fields.program.clone(),
    )
    .mono()
    .placeholder(ctx.t("local_hub.mcp.programNone"))
    .leading(if found { Icon::CircleCheck } else { Icon::File });
    if look.program_refused {
      input = input.invalid();
    } else if let Some(error) = shown(ctx, look, &look.errors.program, &look.typed.program) {
      input = input.error(error);
    }
    let choose = ActionButton::new(
      "local-hub-mcp-choose",
      ButtonKind::Secondary,
      ctx.t("local_hub.mcp.choose"),
    )
    .icon(Icon::FolderOpen)
    .compact()
    .on_click({
      let choose = self.choose.clone();
      move || choose.run(())
    });
    let program = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Row::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .child(input.render(ctx)),
      )
      .child(choose);
    let label = ctx.t("connections.mcp.property.program");
    let label = if look.creating {
      ctx.t_args("team_new.required", [("label", &*label)])
    } else {
      label
    };
    rows::row("local-hub-mcp-row-program", &label, program_sub, program)
  }

  /// *Arguments*, in one line where no argument has a space in it.
  fn arguments_row(&self, ctx: &mut Ctx, look: &Look<'_>) -> Row {
    let arguments = if self.one_line_arguments {
      FieldInput::new(
        "connection-mcp-arguments",
        ctx.t("connections.mcp.property.arguments"),
        self.arguments_line.clone(),
      )
      .mono()
    } else {
      FieldInput::new(
        "connection-mcp-arguments",
        ctx.t("connections.mcp.property.arguments"),
        self.fields.arguments.clone(),
      )
      .mono()
      .compact_text_area()
    };
    let arguments = match shown(ctx, look, &look.errors.arguments, &look.typed.arguments) {
      Some(error) => arguments.error(error),
      None => arguments,
    };
    rows::row(
      "local-hub-mcp-row-arguments",
      &ctx.t("connections.mcp.property.arguments"),
      look.changed(ctx, &look.typed.arguments, &look.saved.arguments),
      arguments.render(ctx),
    )
  }

  /// *Folder*: the worktree unless a path is typed.
  fn folder_row(&self, ctx: &mut Ctx, look: &Look<'_>) -> Row {
    let folder = FieldInput::new(
      "connection-mcp-folder",
      ctx.t("connections.mcp.property.folder"),
      self.fields.folder.clone(),
    )
    .mono()
    .placeholder(ctx.t("connections.mcp.theWorktree"));
    let folder = match shown(ctx, look, &look.errors.working_directory, &look.typed.working_directory) {
      Some(error) => folder.error(error),
      None => folder,
    };
    rows::row(
      "local-hub-mcp-row-folder",
      &ctx.t("connections.mcp.property.folder"),
      look.changed(ctx, &look.typed.working_directory, &look.saved.working_directory),
      folder.render(ctx),
    )
  }

  /// *Variables*, one `NAME=value` a line, with *Add a variable from a secret…* under them.
  fn variables_row(&self, ctx: &mut Ctx, look: &Look<'_>) -> Row {
    let variables = FieldInput::new(
      "connection-mcp-variables",
      ctx.t("connections.mcp.variables"),
      self.fields.variables.clone(),
    )
    .mono()
    .compact_text_area();
    let variables = match shown(ctx, look, &look.errors.variables, &look.typed.variables) {
      Some(error) => variables.error(error),
      None => variables,
    };
    let control = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(variables.render(ctx))
      .child(Row::new().child(self.fields.add_secret(ctx, &look.listed.secrets)));
    rows::row(
      "local-hub-mcp-row-variables",
      &ctx.t("connections.mcp.variables"),
      look.changed(ctx, &look.typed.variables, &look.saved.variables),
      control,
    )
  }

  /// A remote server's endpoint and authorization, as the form draws them.
  fn remote_rows(&self, ctx: &mut Ctx, look: &Look<'_>) -> Vec<Row> {
    let elements = self.fields.remote_elements(ctx, (look.listed, look.errors), look.tried);
    vec![rows::row(
      "local-hub-mcp-row-remote",
      &ctx.t("connections.mcp.property.endpoint"),
      None,
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .with_children(elements),
    )]
  }

  /// *Run without asking*: the tools, written in one line.
  fn tools_row(&self, ctx: &mut Ctx, look: &Look<'_>) -> Row {
    let tools = FieldInput::new(
      "connection-mcp-tools",
      ctx.t("local_hub.mcp.tools"),
      self.tools_line.clone(),
    )
    .mono()
    .placeholder(ctx.t("local_hub.mcp.toolsNone"));
    let tools = match shown(ctx, look, &look.errors.exempt_tools, &look.typed.exempt_tools) {
      Some(error) => tools.error(error),
      None => tools,
    };
    rows::row(
      "local-hub-mcp-row-tools",
      &ctx.t("local_hub.mcp.tools"),
      None,
      tools.render(ctx),
    )
  }
}

impl Editor {
  /// The secret rows, then *On · Runs may use it*.
  pub(super) fn after_rows(&self, ctx: &mut Ctx, look: &Look<'_>, server_on: bool) -> Vec<Row> {
    let mut rows = secrets::rows_of(ctx, &self.form.asking, look.secrets);
    let switch = self.switch.clone();
    rows.push(switch_row(ctx, server_on, move |on| switch.run(on)));
    rows
  }
}

/// *On · Runs may use it* with the switch at `on`; a change of it calls `on_change`.
pub(in crate::screens) fn switch_row(ctx: &Ctx, on: bool, on_change: impl Fn(bool) + Send + Sync + 'static) -> Row {
  let toggle = Toggle::new("local-hub-mcp-on", ToggleKind::Switch, on, ctx.t("local_hub.mcp.on"))
    .hide_label()
    .on_change(on_change);
  let value = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(toggle)
    .child(
      Text::new(&ctx.t("local_hub.mcp.onHint"))
        .variant(HubText::Value.role())
        .flex(1.0),
    );
  rows::row("local-hub-mcp-on-row", &ctx.t("local_hub.mcp.on"), None, value)
}

/// The error under a field, once it holds something or a save was tried.
fn shown(
  ctx: &Ctx,
  look: &Look<'_>,
  error: &Option<orchester_workspace::messages::Message>,
  typed: &str,
) -> Option<std::sync::Arc<str>> {
  error
    .as_ref()
    .filter(|_| look.tried || !typed.trim().is_empty())
    .map(|error| message_text(ctx, error))
}
