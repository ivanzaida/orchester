//! The fields of Add / Edit MCP server as signals the page types into, and its groups (`Server` `YiaYW`, `Approvals`
//! `yg3Oz` in `11 · Add MCP server`): the name, what it runs as (a program here or a remote server, which the agent's
//! CLI starts or reaches), a program's command, arguments, folder and variables, or a remote server's endpoint and
//! authorization (`remote`), and the tools that run without asking (`tools`); each row with one short hint line, the
//! rest in its (i). A variable can take its value from a project secret: *Add a variable from a secret…* appends
//! `NAME=${secret:NAME@environment}` (`euVYY`). Adding a server says what it widens in a warning line (`Widens`
//! `tXVIh`).

mod remote;
mod tools;

use lurq::{
  app::ctx::Ctx,
  components::{Column, Placement, Popup, Row},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::secret::Secret;
use orchester_workspace::{
  connections::{
    data::ProjectConnections,
    mcp::{McpServerErrors, McpServerText, TransportChoice, secret_reference},
  },
  messages::Message,
};

use self::remote::AuthorizationFields;
use crate::{
  screens::project_settings::kit::{FormField, form_group, warning_line},
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    field::FieldInput,
    icon::Icon,
    menu::{MenuItem, menu_panel},
    message::message_text,
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
  },
};

/// The secrets menu's width (`Add a variable from a secret… · menu` in `euVYY`).
const SECRETS_MENU_WIDTH: f32 = 300.0;

/// The page's fields.
#[derive(Clone)]
pub(super) struct Fields {
  name: Signal<String>,
  transport: Signal<TransportChoice>,
  program: Signal<String>,
  arguments: Signal<String>,
  folder: Signal<String>,
  variables: Signal<String>,
  url: Signal<String>,
  authorization: AuthorizationFields,
  exempt_tools: Signal<String>,
  /// The tool name being typed into *Run without asking*.
  tool: Signal<String>,
  /// The secrets menu of Variables.
  secrets_open: Signal<bool>,
  secrets_anchor: ElementRef,
}

impl Fields {
  /// The fields, filled with `text`.
  pub(super) fn new(ctx: &mut Ctx, text: &McpServerText) -> Self {
    Self {
      name: ctx.signal(text.name.clone()),
      transport: ctx.signal(text.transport),
      program: ctx.signal(text.program.clone()),
      arguments: ctx.signal(text.arguments.clone()),
      folder: ctx.signal(text.working_directory.clone()),
      variables: ctx.signal(text.variables.clone()),
      url: ctx.signal(text.url.clone()),
      authorization: AuthorizationFields::new(ctx, &text.authorization),
      exempt_tools: ctx.signal(text.exempt_tools.clone()),
      tool: ctx.signal(String::new()),
      secrets_open: ctx.signal(false),
      secrets_anchor: ctx.element_ref(),
    }
  }

  /// The fields as typed.
  pub(super) fn text(&self) -> McpServerText {
    McpServerText {
      name: self.name.get(),
      transport: self.transport.get(),
      program: self.program.get(),
      arguments: self.arguments.get(),
      working_directory: self.folder.get(),
      variables: self.variables.get(),
      url: self.url.get(),
      authorization: self.authorization.text(),
      exempt_tools: self.exempt_tools.get(),
    }
  }

  /// The page's groups, and the warning line of what adding a server widens (`adding`); an error shows under a field
  /// that holds something, and under every field once a save was `tried`.
  pub(super) fn sections(
    &self,
    ctx: &mut Ctx,
    (listed, errors): (&ProjectConnections, &McpServerErrors),
    tried: bool,
    adding: bool,
  ) -> Vec<Element> {
    let shown = |value: &Signal<String>, error: &Option<Message>| {
      error
        .as_ref()
        .filter(|_| tried || !value.get_untracked().trim().is_empty())
        .map(|error| message_text(ctx, error))
    };
    let secret = errors
      .secret
      .as_ref()
      .filter(|_| self.authorization.secret_shown(tried))
      .map(|error| message_text(ctx, error));
    let errors = Shown {
      name: shown(&self.name, &errors.name),
      program: shown(&self.program, &errors.program),
      arguments: shown(&self.arguments, &errors.arguments),
      folder: shown(&self.folder, &errors.working_directory),
      variables: shown(&self.variables, &errors.variables),
      url: shown(&self.url, &errors.url),
      username: shown(self.authorization.username(), &errors.username),
      header: shown(self.authorization.header(), &errors.header),
      secret,
      exempt_tools: shown(&self.exempt_tools, &errors.exempt_tools),
    };
    let mut rows = self.server_rows(ctx, &errors);
    let transport = self.transport.get();
    rows.extend(match transport {
      TransportChoice::Stdio => self.program_rows(ctx, &errors, &listed.secrets),
      TransportChoice::Http => self.remote_rows(ctx, &errors, &listed.secrets),
    });
    let mut sections = vec![
      form_group(
        "connection-mcp-server",
        &ctx.t("connections.mcp.dialog.server"),
        None,
        rows,
      ),
      self.approvals_section(ctx, &errors),
    ];
    if adding {
      let words = match transport {
        TransportChoice::Stdio => ctx.t("connections.mcp.dialog.widensProgram"),
        TransportChoice::Http => ctx.t("connections.mcp.dialog.widensRemote"),
      };
      sections.push(warning_line(ctx, "connection-mcp-widens", &words, None));
    }
    sections
  }

  /// The name, and what the server runs as (`Runs as` `HQZg7`), which the agent's CLI starts or reaches.
  fn server_rows(&self, ctx: &mut Ctx, errors: &Shown) -> Vec<Element> {
    let name = input(
      ctx,
      ("connection-mcp-name", "connections.mcp.dialog.name"),
      &self.name,
      errors.name.clone(),
    )
    .placeholder(ctx.t("connections.mcp.dialog.namePlaceholder"))
    .mono()
    .render(ctx);
    let name = FormField::new("connection-mcp-name", ctx.t("connections.mcp.dialog.name"))
      .hint(ctx.t("connections.mcp.dialog.nameHintShort"))
      .info(ctx.t("connections.mcp.dialog.nameHint"))
      .render(ctx, name);
    let chosen = self.transport.get();
    let segment = |choice: TransportChoice, (id, key): (&str, &str)| {
      let transport = self.transport.clone();
      Segment::new(
        id.to_owned(),
        SegmentFace::Text(ctx.t(key)),
        chosen == choice,
        move || transport.set(choice),
      )
    };
    let kinds = segmented(
      "connection-mcp-type-choice",
      [
        segment(
          TransportChoice::Stdio,
          ("connection-mcp-stdio", "connections.mcp.dialog.programHere"),
        ),
        segment(
          TransportChoice::Http,
          ("connection-mcp-http", "connections.mcp.dialog.remoteServer"),
        ),
      ],
      Focus::Keyboard,
    );
    let detail = match chosen {
      TransportChoice::Stdio => "connections.mcp.dialog.stdioDetail",
      TransportChoice::Http => "connections.mcp.dialog.httpDetail",
    };
    let kind = FormField::new("connection-mcp-type", ctx.t("connections.mcp.runsAs"))
      .info(ctx.t(detail))
      .render(ctx, Row::new().child(kinds));
    vec![name, kind]
  }

  /// A program's command, its arguments, its folder and its variables.
  fn program_rows(&self, ctx: &mut Ctx, errors: &Shown, secrets: &[Secret]) -> Vec<Element> {
    let program = input(
      ctx,
      ("connection-mcp-program", "connections.mcp.property.program"),
      &self.program,
      errors.program.clone(),
    )
    .placeholder(ctx.t("connections.mcp.dialog.programPlaceholder"))
    .mono()
    .render(ctx);
    let folder = input(
      ctx,
      ("connection-mcp-folder", "connections.mcp.property.folder"),
      &self.folder,
      errors.folder.clone(),
    )
    .placeholder(ctx.t("connections.mcp.theWorktree"))
    .mono()
    .render(ctx);
    let arguments = input(
      ctx,
      ("connection-mcp-arguments", "connections.mcp.property.arguments"),
      &self.arguments,
      errors.arguments.clone(),
    )
    .compact_text_area()
    .mono()
    .render(ctx);
    vec![
      FormField::new("connection-mcp-program", ctx.t("connections.mcp.property.program"))
        .hint(ctx.t("connections.mcp.dialog.programHintShort"))
        .render(ctx, program),
      FormField::new("connection-mcp-arguments", ctx.t("connections.mcp.property.arguments"))
        .hint(ctx.t("connections.mcp.dialog.argumentsHintShort"))
        .render(ctx, arguments),
      FormField::new("connection-mcp-folder", ctx.t("connections.mcp.property.folder"))
        .hint(ctx.t("connections.mcp.dialog.folderHintShort"))
        .info(ctx.t("connections.mcp.dialog.folderHint"))
        .render(ctx, folder),
      self.variables_row(ctx, errors, secrets),
    ]
  }

  fn variables_row(&self, ctx: &mut Ctx, errors: &Shown, secrets: &[Secret]) -> Element {
    let area = input(
      ctx,
      ("connection-mcp-variables", "connections.mcp.variables"),
      &self.variables,
      errors.variables.clone(),
    )
    .compact_text_area()
    .mono()
    .render(ctx);
    let add = self.add_secret(ctx, secrets);
    let control = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(area)
      .child(Row::new().child(add));
    let info = ctx.t_args(
      "connections.mcp.dialog.variablesInfo",
      [
        ("hint", ctx.t("connections.mcp.dialog.variablesHint")),
        ("title", ctx.t("connections.mcp.dialog.secretsTitle")),
        ("detail", ctx.t("connections.mcp.dialog.secretsDetail")),
      ],
    );
    FormField::new("connection-mcp-variables", ctx.t("connections.mcp.variables"))
      .hint(ctx.t("connections.mcp.dialog.variablesHintShort"))
      .info(info)
      .render(ctx, control)
  }

  /// *Add a variable from a secret…*: a menu of the project's secrets, each appending its reference.
  fn add_secret(&self, ctx: &mut Ctx, secrets: &[Secret]) -> Element {
    let label = ctx.t("connections.mcp.dialog.addSecret");
    if secrets.is_empty() {
      return crate::ui::unavailable::with_reason(
        ActionButton::new("connection-mcp-add-secret", ButtonKind::Ghost, label)
          .compact()
          .unavailable(ctx.t("connections.mcp.dialog.noSecrets")),
        "connection-mcp-add-secret-reason",
        &ctx.t("connections.mcp.dialog.noSecrets"),
      )
      .into();
    }
    let open = self.secrets_open.clone();
    let button = ActionButton::new("connection-mcp-add-secret", ButtonKind::Ghost, label)
      .icon(Icon::KeyRound)
      .compact()
      .element_ref(self.secrets_anchor.clone())
      .on_click(move || open.update(|open| *open = !*open));
    let rows: Vec<Element> = secrets
      .iter()
      .enumerate()
      .map(|(index, secret)| {
        let (variables, line) = (
          self.variables.clone(),
          format!("{}={}", secret.name(), secret_reference(secret)),
        );
        let name = ctx.t_args(
          "connections.mcp.secretName",
          [
            ("name", secret.name().as_str()),
            ("environment", secret.environment().as_str()),
          ],
        );
        MenuItem::new(format!("connection-mcp-secret-{index}"), name, move || {
          variables.update(|text| {
            if !text.is_empty() && !text.ends_with('\n') {
              text.push('\n');
            }
            text.push_str(&line);
          });
        })
        .icon(Icon::KeyRound)
        .render(ctx, &self.secrets_open)
      })
      .collect();
    let panel = menu_panel("connection-mcp-secrets")
      .width(Dimension::Px(SECRETS_MENU_WIDTH))
      .with_children(rows);
    // Under the button, from its start.
    let popup = Popup::new(self.secrets_anchor.clone(), panel)
      .open(self.secrets_open.clone())
      .placement(Placement::BottomStart)
      .offset(0.0, ExtraSpacing::Snug.value());
    Row::new().flex_shrink(0.0).child(button).child(popup).into()
  }

  fn approvals_section(&self, ctx: &mut Ctx, errors: &Shown) -> Element {
    let row = self.tools_row(ctx, errors.exempt_tools.clone());
    form_group(
      "connection-mcp-approvals",
      &ctx.t("connections.mcp.dialog.approvals"),
      None,
      vec![row],
    )
  }
}

/// The errors a render shows, in words.
struct Shown {
  name: Option<std::sync::Arc<str>>,
  program: Option<std::sync::Arc<str>>,
  arguments: Option<std::sync::Arc<str>>,
  folder: Option<std::sync::Arc<str>>,
  variables: Option<std::sync::Arc<str>>,
  url: Option<std::sync::Arc<str>>,
  username: Option<std::sync::Arc<str>>,
  header: Option<std::sync::Arc<str>>,
  secret: Option<std::sync::Arc<str>>,
  exempt_tools: Option<std::sync::Arc<str>>,
}

/// A text field bound to `value`, named by `key`, outlined with `error` under it when there is one.
fn input(ctx: &Ctx, (id, key): (&str, &str), value: &Signal<String>, error: Option<std::sync::Arc<str>>) -> FieldInput {
  let field = FieldInput::new(id.to_owned(), ctx.t(key), value.clone());
  match error {
    Some(error) => field.error(error),
    None => field,
  }
}
