//! The status bar (`Status bar` `sMpGe`): what the app knows about itself on the right, the app's MCP
//! server and its version. The project, branch, runner, agents, waiting runs, tokens and plan usage of
//! the design come with their data; until then they are not shown.

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use crate::{
  mcp::McpStatus,
  theme::{ExtraSpacing, IconSize},
  ui::icon::{Icon, icon},
};

/// `Status bar` height.
const STATUS_BAR_HEIGHT: f32 = 28.0;
/// `Status item` height.
const STATUS_ITEM_HEIGHT: f32 = 26.0;

pub struct StatusBar;

impl Component for StatusBar {
  type Props = ();

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let status = ctx
      .use_context::<McpStatus>()
      .expect("the root provides the MCP status before mounting the shell");
    let mcp = match status {
      McpStatus::Listening { port, .. } => ctx.t_args("status.mcpListening", [("port", port)]),
      McpStatus::Unavailable => ctx.t("status.mcpUnavailable"),
    };
    // Agents may click and type in the window: said while it lasts, whatever decided it.
    let window_control = matches!(
      status,
      McpStatus::Listening {
        window_control: true,
        ..
      }
    )
    .then(|| {
      item(
        "status-window-control",
        Icon::MousePointerClick,
        &ctx.t("status.windowControl"),
      )
    });
    let version = ctx.t_args("status.version", [("version", env!("CARGO_PKG_VERSION"))]);
    Row::new()
      .id("status-bar")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(STATUS_BAR_HEIGHT))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .align_items(Alignment::Center)
      .child(Row::new().flex(1.0))
      .with_children(window_control)
      .child(item("status-mcp", Icon::RadioTower, &mcp))
      .child(item("status-version", Icon::Sparkles, &version))
  }
}

/// A status item (`Status item` `wnnsD`): icon and label.
fn item(id: &'static str, glyph: Icon, label: &str) -> Row {
  Row::new()
    .id(id)
    .height(Dimension::Px(STATUS_ITEM_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Sm, PaletteColor::TextMuted))
    .child(Text::new(label).variant(TypographyStyle::Caption).nowrap())
}
