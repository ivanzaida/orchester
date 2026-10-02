//! Fields of the MCP server dialog (`cJGKL` in 04.E `yRb2k`, 04.E2 `Qvhc6`; errors as in 04.E3
//! `Hpycl`): name and type, transport and who starts or reaches the server, then a program's file,
//! folder, arguments and variables, or a URL and its bearer token, then the tools that run without
//! asking (no frame draws them; they take the arguments' area). Where a secret goes is said in a
//! notice.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Select, SelectOption, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use super::{
  super::secrets::labels::environment_key,
  DialogMode,
  form::{fixed, pair, select_field, type_field},
  mcp_dialog::{BearerChoice, InsertSecret, McpDialog},
};
use crate::{
  connections::mcp::{McpServerErrors, TransportChoice},
  messages::Message,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, select_button},
    field::{FieldNote, field, select_style, styled_input, text_input},
    icon::Icon,
    message_text,
    notice::Notice,
  },
};

/// Field height (`Iitmk`), the lines the argument and variable areas show before they scroll, and
/// the height of *Add a variable from a secret…* (`Pdcuk`).
const FIELD_HEIGHT: f32 = 40.0;
const AREA_ROWS: (usize, usize) = (3, 8);
const ADD_SECRET_HEIGHT: f32 = 30.0;
/// Reusable select menu `u0ByM`, wider than the secret button `Pdcuk`.
const ADD_SECRET_MENU_WIDTH: f32 = 260.0;

impl McpDialog {
  pub(super) fn form(&self, ctx: &Ctx, errors: &McpServerErrors) -> Element {
    let editing = matches!(self.context.mode, DialogMode::EditMcp(_));
    let transport = self.transport.get();
    // A program is started by the agent's CLI, a remote server reached by it (04.E, 04.E2 `Qvhc6`).
    let (reached_label, reached) = match transport {
      TransportChoice::Stdio => ("connections.mcp.property.startedBy", "connections.mcp.startedByCli"),
      TransportChoice::Http => ("connections.mcp.property.reachedBy", "connections.mcp.reachedByCli"),
    };
    // The secret button's focus ring reaches past the form's columns into its padding.
    let column = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Form.role())
      .overflow_visible()
      .child(pair(
        self.name_field(ctx, errors.name.as_ref()),
        type_field(ctx, &self.kind, editing),
      ))
      .child(pair(
        self.transport_field(ctx),
        fixed(ctx, "mcp-started-by", reached_label, reached),
      ));
    let column = match transport {
      TransportChoice::Stdio => column.with_children(self.stdio_fields(ctx, errors)),
      TransportChoice::Http => column.with_children(self.http_fields(ctx, errors)),
    };
    column
      .child(area_field(
        ctx,
        (
          "mcp-exempt-tools",
          "connections.mcp.exemptTools",
          hint(ctx, "connections.mcp.dialog.exemptToolsHint"),
        ),
        &self.exempt_tools,
        errors.exempt_tools.as_ref(),
      ))
      .child(Notice::new(
        Icon::ShieldCheck,
        ctx.t("connections.mcp.dialog.secretsTitle"),
        ctx.t("connections.mcp.dialog.secretsDetail"),
      ))
      .child(Text::new(&ctx.t("connections.mcp.dialog.nextStep")).variant(ExtraTypography::Explanation.role()))
      .into()
  }

  fn name_field(&self, ctx: &Ctx, error: Option<&Message>) -> Element {
    let input = text_input(
      ctx,
      "mcp-name",
      self.name.clone(),
      error.is_some() && !self.name.get().is_empty(),
      TypographyStyle::FieldLabel,
    )
    .height(Dimension::Px(FIELD_HEIGHT))
    .placeholder(&ctx.t("connections.mcp.dialog.namePlaceholder"));
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(field(
        &ctx.t("connections.dialog.name"),
        input.into(),
        note(ctx, error, &self.name),
      ))
      .child(hint(ctx, "connections.mcp.dialog.nameHint"))
      .into()
  }

  fn transport_field(&self, ctx: &Ctx) -> Element {
    let options = [TransportChoice::Stdio, TransportChoice::Http].map(|choice| {
      let key = match choice {
        TransportChoice::Stdio => "connections.mcp.typeStdio",
        TransportChoice::Http => "connections.mcp.typeHttp",
      };
      (choice, ctx.t(key))
    });
    select_field(
      ctx,
      "mcp-transport",
      &ctx.t("connections.mcp.property.type"),
      self.transport.clone(),
      options,
    )
  }

  fn stdio_fields(&self, ctx: &Ctx, errors: &McpServerErrors) -> Vec<Element> {
    let program = text_input(
      ctx,
      "mcp-program",
      self.program.clone(),
      errors.program.is_some() && !self.program.get().is_empty(),
      ExtraTypography::MonoField.role(),
    )
    .height(Dimension::Px(FIELD_HEIGHT))
    .placeholder(&ctx.t("connections.mcp.dialog.programPlaceholder"));
    let folder = text_input(
      ctx,
      "mcp-folder",
      self.working_directory.clone(),
      errors.working_directory.is_some(),
      ExtraTypography::MonoField.role(),
    )
    .height(Dimension::Px(FIELD_HEIGHT))
    .placeholder(&ctx.t("connections.mcp.theWorktree"));
    let program = field(
      &ctx.t("connections.mcp.property.program"),
      program.into(),
      note(ctx, errors.program.as_ref(), &self.program),
    );
    let folder = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(field(
        &ctx.t("connections.mcp.property.folder"),
        folder.into(),
        note(ctx, errors.working_directory.as_ref(), &self.working_directory),
      ))
      .child(hint(ctx, "connections.mcp.dialog.folderHint"));
    vec![
      pair(program, folder.into()),
      area_field(
        ctx,
        (
          "mcp-arguments",
          "connections.mcp.property.arguments",
          hint(ctx, "connections.mcp.dialog.argumentsHint"),
        ),
        &self.arguments,
        errors.arguments.as_ref(),
      ),
      area_field(
        ctx,
        ("mcp-variables", "connections.mcp.variables", self.variables_hint(ctx)),
        &self.variables,
        errors.variables.as_ref(),
      ),
    ]
  }

  /// The variables hint with *Add a variable from a secret…* at its right (`w6jcyL`).
  fn variables_hint(&self, ctx: &Ctx) -> Element {
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .overflow_visible()
      .child(
        Column::new()
          .flex(1.0)
          .child(hint(ctx, "connections.mcp.dialog.variablesHint")),
      )
      .child(self.insert_secret(ctx))
      .into()
  }

  /// *Add a variable from a secret…* (`Pdcuk`): each option shows `NAME (environment)` on one
  /// line in the wider menu (`u0ByM`). The chosen secret becomes a variable reference.
  fn insert_secret(&self, ctx: &Ctx) -> Element {
    if self.context.secrets.is_empty() {
      return ActionButton::new(ctx.t("connections.mcp.dialog.noSecrets"))
        .icon(Icon::KeyRound)
        .compact()
        .id("mcp-add-secret")
        .height(Dimension::Px(ADD_SECRET_HEIGHT))
        .disabled()
        .render(ctx);
    }
    let options = self
      .context
      .secrets
      .iter()
      .map(|secret| {
        let environment = ctx.t(environment_key(secret.environment()));
        let label = ctx.t_args(
          "connections.mcp.secretName",
          [("name", secret.name().as_str()), ("environment", environment.as_ref())],
        );
        SelectOption::new(InsertSecret(Some(secret.id())), label)
      })
      .collect();
    let face = (Icon::KeyRound, ctx.t("connections.mcp.dialog.addSecret"));
    Element::from(select_button(
      ctx,
      self.insert.clone(),
      options,
      face,
      ADD_SECRET_HEIGHT,
      ADD_SECRET_MENU_WIDTH,
    ))
    .id("mcp-add-secret")
  }

  fn http_fields(&self, ctx: &Ctx, errors: &McpServerErrors) -> Vec<Element> {
    let url = text_input(
      ctx,
      "mcp-url",
      self.url.clone(),
      errors.url.is_some() && !self.url.get().is_empty(),
      ExtraTypography::MonoField.role(),
    )
    .height(Dimension::Px(FIELD_HEIGHT))
    .placeholder(&ctx.t("connections.mcp.dialog.urlPlaceholder"));
    let mut options: Vec<(BearerChoice, Arc<str>)> =
      vec![(BearerChoice(None), ctx.t("connections.mcp.noAuthentication"))];
    options.extend(self.context.secrets.iter().map(|secret| {
      let name = ctx.t_args(
        "connections.mcp.secretName",
        [
          ("name", secret.name().as_str()),
          ("environment", secret.environment().as_str()),
        ],
      );
      (
        BearerChoice(Some(secret.id())),
        ctx.t_args("connections.mcp.bearer", [("secret", &*name)]),
      )
    }));
    let bearer = Select::new(self.bearer.clone())
      .options(options)
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(FIELD_HEIGHT))
      .style(select_style(ctx, FIELD_HEIGHT));
    let bearer = Column::new()
      .id("mcp-bearer")
      .width(Dimension::Pct(100.0))
      .child(bearer);
    vec![
      field(
        &ctx.t("connections.mcp.property.endpoint"),
        url.into(),
        note(ctx, errors.url.as_ref(), &self.url),
      ),
      field(
        &ctx.t("connections.property.authentication"),
        bearer.into(),
        FieldNote::None,
      ),
    ]
  }
}

/// A field's error once something was entered: an empty field says why in the save bar only.
fn note(ctx: &Ctx, error: Option<&Message>, value: &Signal<String>) -> FieldNote {
  match error {
    Some(error) if !value.get().trim().is_empty() => FieldNote::Error(message_text(ctx, error)),
    _ => FieldNote::None,
  }
}

fn hint(ctx: &Ctx, key: &str) -> Element {
  Text::new(&ctx.t(key)).variant(ExtraTypography::Hint.role()).into()
}

/// A mono area drawn like a field, lines of `value`, with its label, error and `hint`, as Project
/// settings › Environment draws its variables (`BGMgO`).
fn area_field(
  ctx: &Ctx,
  (id, label, hint): (&'static str, &str, Element),
  value: &Signal<String>,
  error: Option<&Message>,
) -> Element {
  let input = styled_input(ctx, value.clone(), ExtraTypography::MonoArea.role())
    .id(id)
    .textarea()
    .rows(AREA_ROWS.0, AREA_ROWS.1)
    .width(Dimension::Pct(100.0));
  let area = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Stack.role())
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(
      BorderSize::Sm,
      if error.is_some() {
        PaletteColor::BorderStrong
      } else {
        PaletteColor::Border
      },
    ))
    .rounded(RadiusSize::Md)
    .child(input);
  let note = error.map_or(FieldNote::None, |error| FieldNote::Error(message_text(ctx, error)));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .overflow_visible()
    .child(field(&ctx.t(label), area.into(), note))
    .child(hint)
    .into()
}
