//! The fields of Add / Edit MCP server as signals the page types into, and its sections (`Section · Server`,
//! `Program`, `Approvals` in `M20DGB`): the name and type, who starts or reaches the server (fixed: the
//! agent's CLI), a program's command, folder, arguments and variables, or a remote server's endpoint and
//! bearer secret, and the tools that run without asking. A variable can take its value from a project
//! secret: *Add a variable from a secret…* appends `NAME=${secret:NAME@environment}` (`euVYY`).

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Placement, Popup, Row, SelectOption, Spacer, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::SecretId, secret::Secret};
use orchester_workspace::{
  connections::{
    data::ProjectConnections,
    mcp::{McpServerErrors, McpServerText, TransportChoice, secret_reference},
  },
  messages::Message,
};

use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, SelectField},
    form_section::{Label, Place, section, stacked_row},
    icon::{Icon, icon},
    menu::{MenuItem, menu_panel},
    message::message_text,
    notice::{Notice, NoticeTone},
    option_card::OptionCard,
  },
};

/// The Authentication select's choice: a bearer token from a project secret, or none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Bearer(Option<SecretId>);

impl DevtoolsInspectable for Bearer {}

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
  bearer: Signal<Bearer>,
  exempt_tools: Signal<String>,
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
      bearer: ctx.signal(Bearer(text.bearer)),
      exempt_tools: ctx.signal(text.exempt_tools.clone()),
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
      bearer: self.bearer.get().0,
      exempt_tools: self.exempt_tools.get(),
    }
  }

  /// The page's sections; an error shows under a field that holds something, and under every field once a
  /// save was `tried`.
  pub(super) fn sections(
    &self,
    ctx: &mut Ctx,
    (listed, errors): (&ProjectConnections, &McpServerErrors),
    tried: bool,
    project: &str,
  ) -> Vec<Element> {
    let shown = |value: &Signal<String>, error: &Option<Message>| {
      error
        .as_ref()
        .filter(|_| tried || !value.get_untracked().trim().is_empty())
        .map(|error| message_text(ctx, error))
    };
    let errors = Shown {
      name: shown(&self.name, &errors.name),
      program: shown(&self.program, &errors.program),
      arguments: shown(&self.arguments, &errors.arguments),
      folder: shown(&self.folder, &errors.working_directory),
      variables: shown(&self.variables, &errors.variables),
      url: shown(&self.url, &errors.url),
      exempt_tools: shown(&self.exempt_tools, &errors.exempt_tools),
    };
    let reach = match self.transport.get() {
      TransportChoice::Stdio => self.program_section(ctx, &errors, &listed.secrets),
      TransportChoice::Http => self.remote_section(ctx, &errors, &listed.secrets),
    };
    vec![
      self.server_section(ctx, &errors),
      reach,
      self.approvals_section(ctx, &errors),
      closing(ctx, project),
    ]
  }

  fn server_section(&self, ctx: &mut Ctx, errors: &Shown) -> Element {
    let name = input(
      ctx,
      ("connection-mcp-name", "connections.mcp.dialog.name"),
      &self.name,
      errors.name.clone(),
    )
    .placeholder(ctx.t("connections.mcp.dialog.namePlaceholder"))
    .mono()
    .render(ctx);
    let name = stacked_row(
      Label::new("connection-mcp-name", ctx.t("connections.mcp.dialog.name"))
        .description(ctx.t("connections.mcp.dialog.nameHint")),
      name,
      Place::Within,
    );
    let chosen = self.transport.get();
    let card = |choice: TransportChoice, (id, glyph, title, detail): (&str, Icon, &str, &str)| -> Element {
      let transport = self.transport.clone();
      Column::new()
        .flex(1.0)
        .child(OptionCard::new(
          id.to_owned(),
          (glyph, ctx.t(title)),
          ctx.t(detail),
          chosen == choice,
          move || {
            transport.set(choice);
          },
        ))
        .into()
    };
    let kinds = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(card(
        TransportChoice::Stdio,
        (
          "connection-mcp-stdio",
          Icon::Terminal,
          "connections.mcp.typeStdio",
          "connections.mcp.dialog.stdioDetail",
        ),
      ))
      .child(card(
        TransportChoice::Http,
        (
          "connection-mcp-http",
          Icon::Globe,
          "connections.mcp.typeHttp",
          "connections.mcp.dialog.httpDetail",
        ),
      ));
    let kind = stacked_row(
      Label::new("connection-mcp-type", ctx.t("connections.mcp.property.type")),
      kinds,
      Place::Within,
    );
    let (label, value) = match chosen {
      TransportChoice::Stdio => ("connections.mcp.property.startedBy", "connections.mcp.startedByCli"),
      TransportChoice::Http => ("connections.mcp.property.reachedBy", "connections.mcp.reachedByCli"),
    };
    section(
      "connection-mcp-server",
      &ctx.t("connections.mcp.dialog.server"),
      None,
      vec![name, kind, fixed_row(ctx, label, value)],
    )
    .into()
  }

  fn program_section(&self, ctx: &mut Ctx, errors: &Shown, secrets: &[Secret]) -> Element {
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
    .text_area()
    .mono()
    .render(ctx);
    let rows = vec![
      stacked_row(
        Label::new("connection-mcp-program", ctx.t("connections.mcp.property.program"))
          .description(ctx.t("connections.mcp.dialog.programHint")),
        program,
        Place::Within,
      ),
      stacked_row(
        Label::new("connection-mcp-folder", ctx.t("connections.mcp.property.folder"))
          .description(ctx.t("connections.mcp.dialog.folderHint")),
        folder,
        Place::Within,
      ),
      stacked_row(
        Label::new("connection-mcp-arguments", ctx.t("connections.mcp.property.arguments"))
          .description(ctx.t("connections.mcp.dialog.argumentsHint")),
        arguments,
        Place::Within,
      ),
      self.variables_row(ctx, errors, secrets),
    ];
    section(
      "connection-mcp-program-section",
      &ctx.t("connections.mcp.property.program"),
      None,
      rows,
    )
    .into()
  }

  fn variables_row(&self, ctx: &mut Ctx, errors: &Shown, secrets: &[Secret]) -> Element {
    let area = input(
      ctx,
      ("connection-mcp-variables", "connections.mcp.variables"),
      &self.variables,
      errors.variables.clone(),
    )
    .text_area()
    .mono()
    .render(ctx);
    let add = self.add_secret(ctx, secrets);
    let under = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Start)
      .child(
        Text::new(&ctx.t("connections.mcp.dialog.variablesHint"))
          .variant(ExtraTypography::Note.role())
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .child(add);
    let control = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(area)
      .child(under);
    stacked_row(
      Label::new("connection-mcp-variables", ctx.t("connections.mcp.variables")),
      control,
      Place::Last,
    )
  }

  /// *Add a variable from a secret…*: a menu of the project's secrets, each appending its reference.
  fn add_secret(&self, ctx: &mut Ctx, secrets: &[Secret]) -> Element {
    let label = ctx.t("connections.mcp.dialog.addSecret");
    if secrets.is_empty() {
      return crate::ui::unavailable::with_reason(
        ActionButton::new("connection-mcp-add-secret", ButtonKind::Secondary, label)
          .unavailable(ctx.t("connections.mcp.dialog.noSecrets")),
        "connection-mcp-add-secret-reason",
        &ctx.t("connections.mcp.dialog.noSecrets"),
      )
      .into();
    }
    let open = self.secrets_open.clone();
    let button = ActionButton::new("connection-mcp-add-secret", ButtonKind::Secondary, label)
      .icon(Icon::KeyRound)
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
    // Under the button, its end at the button's end (`euVYY`).
    let popup = Popup::new(self.secrets_anchor.clone(), panel)
      .open(self.secrets_open.clone())
      .placement(Placement::BottomEnd)
      .offset(0.0, ExtraSpacing::Snug.value());
    Row::new().flex_shrink(0.0).child(button).child(popup).into()
  }

  fn remote_section(&self, ctx: &mut Ctx, errors: &Shown, secrets: &[Secret]) -> Element {
    let url = input(
      ctx,
      ("connection-mcp-url", "connections.mcp.property.endpoint"),
      &self.url,
      errors.url.clone(),
    )
    .placeholder(ctx.t("connections.mcp.dialog.urlPlaceholder"))
    .mono()
    .render(ctx);
    let mut options = vec![SelectOption::new(
      Bearer(None),
      ctx.t("connections.mcp.noAuthentication").to_string(),
    )];
    options.extend(secrets.iter().map(|secret| {
      let name = ctx.t_args(
        "connections.mcp.secretName",
        [
          ("name", secret.name().as_str()),
          ("environment", secret.environment().as_str()),
        ],
      );
      SelectOption::new(
        Bearer(Some(secret.id())),
        ctx.t_args("connections.mcp.bearer", [("secret", &*name)]).to_string(),
      )
    }));
    let bearer = SelectField::new(
      "connection-mcp-bearer",
      ctx.t("connections.property.authentication"),
      self.bearer.clone(),
      options,
    )
    .render(ctx);
    let rows = vec![
      stacked_row(
        Label::new("connection-mcp-url", ctx.t("connections.mcp.property.endpoint")),
        url,
        Place::Within,
      ),
      stacked_row(
        Label::new("connection-mcp-bearer", ctx.t("connections.property.authentication"))
          .description(ctx.t("connections.mcp.dialog.bearerHint")),
        bearer,
        Place::Last,
      ),
    ];
    section(
      "connection-mcp-remote-section",
      &ctx.t("connections.mcp.typeHttp"),
      None,
      rows,
    )
    .into()
  }

  fn approvals_section(&self, ctx: &mut Ctx, errors: &Shown) -> Element {
    let tools = input(
      ctx,
      ("connection-mcp-exempt-tools", "connections.mcp.exemptTools"),
      &self.exempt_tools,
      errors.exempt_tools.clone(),
    )
    .text_area()
    .mono()
    .render(ctx);
    let row = stacked_row(
      Label::new("connection-mcp-exempt-tools", ctx.t("connections.mcp.exemptTools"))
        .description(ctx.t("connections.mcp.dialog.exemptToolsHint")),
      tools,
      Place::Last,
    );
    section(
      "connection-mcp-approvals",
      &ctx.t("connections.mcp.dialog.approvals"),
      None,
      vec![row],
    )
    .into()
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

/// A fixed choice: who starts or reaches the server (`Started by` with its lock in `M20DGB`).
fn fixed_row(ctx: &Ctx, label: &str, value: &str) -> Element {
  Row::new()
    .id("connection-mcp-started-by")
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(Text::new(&ctx.t(label)).variant(ExtraTypography::Strong.role()))
    .child(Spacer::new().flex(1.0))
    .child(icon(Icon::Lock, IconSize::Sm, PaletteColor::TextMuted))
    .child(Text::new(&ctx.t(value)).variant(TypographyStyle::Description).nowrap())
    .into()
}

/// Where secret values go, and what comes after saving.
fn closing(ctx: &Ctx, project: &str) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(
      Notice::new(
        "connection-mcp-secrets-notice",
        NoticeTone::Info,
        ctx.t("connections.mcp.dialog.secretsTitle"),
      )
      .icon(Icon::ShieldCheck)
      .body(ctx.t("connections.mcp.dialog.secretsDetail")),
    )
    .child(
      Text::new(&ctx.t_args("connections.mcp.dialog.nextStep", [("project", project)]))
        .variant(ExtraTypography::Prose.role())
        .color(PaletteColor::TextMuted),
    )
    .into()
}
