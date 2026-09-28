//! Selected MCP server's details (`OXa0o` in 04.D `QdLRd`): name, whether it is on, how the agent's
//! CLI reaches it (type, program or endpoint, arguments, folder), how it authenticates, its
//! variables (a secret by its name only), its tools and the profiles that give it to their agents.
//! Orchester never talks to a server itself, so *Refresh tools* and *Test connection* are shown
//! unavailable, with why; the actions pinned below are *Edit connection* and turning the server on
//! or off. Laid out like the agent connection inspector (`JuVaA`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, ScrollVertical, Text},
  layout::layout_kind::ScrollState,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  connection::mcp::{McpEnvValue, McpServer, McpTransport},
  id::ResourceId,
  secret::Secret,
};

use super::list::mcp_status;
use crate::{
  connections::data::ProfileUse,
  messages::Message,
  screens::project::page::TOPBAR_BUTTON_HEIGHT,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, gutter_scrollbar},
  ui::{
    button::ActionButton,
    icon::Icon,
    link::Link,
    message_text,
    property::{ValueStyle, property, styled_property},
    status_label::status_label as label,
  },
};

/// Inspector width (`OXa0o`).
const INSPECTOR_WIDTH: f32 = 344.0;

type Handler = Arc<dyn Fn() + Send + Sync>;

pub struct McpActions {
  /// Turns the server off when it is on, on when it is off; `None` while other work runs.
  pub toggle: Option<Handler>,
  pub edit: Handler,
  /// A link to a profile that uses it.
  pub profile: Arc<dyn Fn(ResourceId) -> Link + Send + Sync>,
}

pub struct McpInspector<'a> {
  pub server: &'a McpServer,
  pub uses: Vec<&'a ProfileUse>,
  pub secrets: &'a [Secret],
  /// Turning it on or off runs.
  pub working: bool,
  /// Why the last change could not be stored.
  pub error: Option<Message>,
  /// Where the details are scrolled.
  pub scroll: &'a ScrollState,
}

impl McpInspector<'_> {
  pub fn render(&self, ctx: &Ctx, actions: &McpActions) -> Element {
    let (status_icon, status_key) = mcp_status(self.server);
    let details = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Step.role())
      .child(Text::new(self.server.name().as_str()).variant(ExtraTypography::InspectorTitle.role()))
      .child(label(status_icon, &ctx.t(status_key)))
      .with_children(self.properties(ctx))
      .with_children(self.variables(ctx))
      .child(heading(ctx, "connections.mcp.tools"))
      .child(note(ctx, "connections.mcp.toolsNote"))
      .child(button(
        ctx,
        "mcp-refresh-tools",
        "connections.mcp.refreshTools",
        Icon::RefreshCw,
        None,
      ))
      .child(self.used_by(ctx, &actions.profile));
    let toggle_key = if self.server.enabled() {
      "connections.mcp.turnOff"
    } else {
      "connections.mcp.turnOn"
    };
    let detail = match &self.error {
      Some(error) => message_text(ctx, error),
      None => ctx.t("connections.mcp.detail"),
    };
    let pinned = Column::new()
      .width(Dimension::Pct(100.0))
      .padding_right(ExtraSpacing::Stack.role())
      .spacing(ExtraSpacing::Step.role())
      .child(button(ctx, "mcp-test", "connections.test", Icon::Activity, None))
      .child(button(
        ctx,
        "edit-connection",
        "connections.edit",
        Icon::Settings,
        Some(actions.edit.clone()),
      ))
      .child(button(
        ctx,
        "mcp-toggle",
        toggle_key,
        Icon::Power,
        actions.toggle.clone().filter(|_| !self.working),
      ))
      .child(
        Text::new(&detail)
          .variant(ExtraTypography::InspectorNote.role())
          .id("connection-status-detail"),
      );
    Column::new()
      .id("connection-inspector")
      .width(Dimension::Px(INSPECTOR_WIDTH))
      .height(Dimension::Pct(100.0))
      .padding_vertical(SpacingSize::Lg)
      .padding_left(SpacingSize::Lg)
      .padding_right(ExtraSpacing::Stack.role())
      .spacing(ExtraSpacing::Step.role())
      .background(PaletteColor::SurfacePanel)
      .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(
        ScrollVertical::new(details)
          .id("connection-inspector-details")
          .with_scroll_state(self.scroll.clone())
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .scrollbar(gutter_scrollbar()),
      )
      .child(pinned)
      .into()
  }

  /// Type, what runs or where, and how it authenticates (`uvJAu`, `LvBSE`, `u3eWqy`).
  fn properties(&self, ctx: &Ctx) -> Vec<Element> {
    let row = |key: &str, value: &str| property(&ctx.t(key), value);
    match self.server.transport() {
      McpTransport::Stdio(stdio) => {
        let args: Vec<&str> = stdio.args().iter().map(|arg| arg.as_str()).collect();
        // A path in mono, as the program; the default in words.
        let folder = match stdio.working_directory() {
          Some(folder) => styled_property(
            &ctx.t("connections.mcp.property.folder"),
            folder.as_str(),
            ValueStyle::Path,
          ),
          None => row("connections.mcp.property.folder", &ctx.t("connections.mcp.theWorktree")),
        };
        vec![
          row("connections.mcp.property.type", &ctx.t("connections.mcp.typeStdio")),
          styled_property(
            &ctx.t("connections.mcp.property.program"),
            stdio.program().as_str(),
            ValueStyle::Path,
          ),
          styled_property(
            &ctx.t("connections.mcp.property.arguments"),
            &args.join(" "),
            ValueStyle::Path,
          ),
          folder,
          row(
            "connections.mcp.property.startedBy",
            &ctx.t("connections.mcp.startedByCli"),
          ),
        ]
      }
      McpTransport::Http(http) => {
        let authentication = match http.bearer() {
          Some(secret) => ctx.t_args("connections.mcp.bearer", [("secret", &*self.secret_name(ctx, secret))]),
          None => ctx.t("connections.mcp.noAuthentication"),
        };
        vec![
          row("connections.mcp.property.type", &ctx.t("connections.mcp.typeHttp")),
          row("connections.mcp.property.endpoint", http.url().host()),
          row("connections.property.authentication", &authentication),
        ]
      }
    }
  }

  /// The variables a program starts with; a secret's value is never shown, only which secret.
  fn variables(&self, ctx: &Ctx) -> Option<Element> {
    let environment = self.server.environment();
    if environment.is_empty() {
      return None;
    }
    let rows = environment.iter().map(|(name, value)| {
      let value = match value {
        McpEnvValue::Text(text) => text.as_str().into(),
        McpEnvValue::Secret(secret) => ctx.t_args(
          "connections.mcp.fromSecret",
          [("secret", &*self.secret_name(ctx, *secret))],
        ),
      };
      styled_property(name.as_str(), &value, ValueStyle::Mono)
    });
    Some(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Stack.role())
        .child(heading(ctx, "connections.mcp.variables"))
        .with_children(rows)
        .into(),
    )
  }

  /// The secret's name and environment, or that the project no longer lists it.
  fn secret_name(&self, ctx: &Ctx, id: orchester_core::id::SecretId) -> Arc<str> {
    self.secrets.iter().find(|secret| secret.id() == id).map_or_else(
      || ctx.t("connections.mcp.secretGone"),
      |secret| {
        ctx.t_args(
          "connections.mcp.secretName",
          [
            ("name", secret.name().as_str()),
            ("environment", secret.environment().as_str()),
          ],
        )
      },
    )
  }

  /// Read-only links to the profiles that give the server to their agents (`c3NuQu`).
  fn used_by(&self, ctx: &Ctx, profile: &Arc<dyn Fn(ResourceId) -> Link + Send + Sync>) -> Element {
    let links: Vec<Element> = self
      .uses
      .iter()
      .map(|used| {
        ActionButton::new(ctx.t_args("connections.mcp.usedByLink", [("role", &*used.role)]))
          .icon(Icon::ExternalLink)
          .fill_width()
          .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT))
          .link(profile(used.profile))
          .render(ctx)
      })
      .collect();
    // Heading and links 20 apart, as the inspector's other parts (`c3NuQu`, `p4Dya`, `rBsPW`).
    let column = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Step.role())
      .child(heading(ctx, "connections.usedBy"));
    if links.is_empty() {
      return column.child(note(ctx, "connections.mcp.notUsed")).into();
    }
    column.with_children(links).into()
  }
}

fn heading(ctx: &Ctx, key: &str) -> Element {
  Text::new(&ctx.t(key)).variant(TypographyStyle::Label).into()
}

fn note(ctx: &Ctx, key: &str) -> Element {
  Text::new(&ctx.t(key))
    .variant(ExtraTypography::InspectorNote.role())
    .into()
}

/// A full-width secondary button; without a handler it cannot be pressed (unavailable, or waiting
/// for other work).
fn button(ctx: &Ctx, id: &'static str, key: &str, glyph: Icon, handler: Option<Handler>) -> Element {
  let button = ActionButton::new(ctx.t(key))
    .icon(glyph)
    .id(id)
    .fill_width()
    .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT));
  match handler {
    Some(handler) => button.on_click(move || handler()).render(ctx),
    None => button.on_click(|| {}).enabled(false).render(ctx),
  }
}
