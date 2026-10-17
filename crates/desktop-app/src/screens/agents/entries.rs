//! Agent list master NA7sg: 60 high, with mCvPL's five columns.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::agents::{
  data::{ProfileEntry, ProjectAgents, VersionState},
  form::stored_effort,
};

use super::{AgentsNav, Shown, words};
use crate::{
  routes::AgentTab,
  theme::{ExtraColor, ExtraSpacing, IconSize},
  ui::{
    avatar::profile_avatar,
    icon::{Icon, icon},
    pressable::opening,
    table::{ColumnWidth, table_header},
    tooltip::Truncated,
  },
};

pub const ROW_HEIGHT: f32 = 60.0;
pub const TABLE_CHROME: f32 = 69.0;
const CONNECTION: f32 = 190.0;
const PERMISSIONS: f32 = 240.0;
const VERSION: f32 = 170.0;
const USED: f32 = 120.0;

#[derive(Clone)]
pub struct RowProps {
  pub entry: ProfileEntry,
  pub listed: ProjectAgents,
  pub(super) nav: AgentsNav,
}
impl PartialEq for RowProps {
  fn eq(&self, other: &Self) -> bool {
    self.entry == other.entry && self.listed == other.listed
  }
}
impl DevtoolsInspectable for RowProps {}
pub struct AgentRow;
impl Component for AgentRow {
  type Props = RowProps;
  fn create(_: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RowProps>().clone();
    let entry = &props.entry;
    let profile = &entry.profile;
    let connection = props.listed.connections.iter().find(|c| c.id() == profile.connection());
    let connection_name = connection.map_or_else(|| ctx.t("agents.missingConnection"), |c| c.name().as_str().into());
    let model = match stored_effort(profile) {
      Some(effort) => ctx.t_args(
        "agents.modelEffort",
        [("model", profile.model().as_str()), ("effort", &effort)],
      ),
      None => profile.model().as_str().into(),
    };
    let count = props.listed.usage.count(entry.id);
    let used = if count == 0 {
      ctx.t("agents.notUsed")
    } else {
      ctx.t_args("agents.pipelineCount", [("count", count.to_string())])
    };
    let id = entry.id;
    let nav = props.nav;
    let title = Truncated::new(
      format!("agent-name-{id}"),
      profile.role().as_str(),
      TypographyStyle::Label,
    )
    .render(ctx);
    let responsibility = Truncated::new(
      format!("agent-detail-{id}"),
      profile.responsibilities(),
      TypographyStyle::Description,
    )
    .render(ctx);
    let row = Row::new()
      .height(Dimension::Px(ROW_HEIGHT))
      .width(Dimension::Pct(100.0))
      .padding(Padding::horizontal(SpacingSize::Md))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(
        Row::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(ExtraSpacing::Loose.role())
          .align_items(Alignment::Center)
          .child(profile_avatar(&format!("agent-avatar-{id}"), profile.role().as_str()))
          .child(
            Column::new()
              .flex(1.0)
              .min_width(Dimension::Px(0.0))
              .spacing(ExtraSpacing::Tight.role())
              .child(title)
              .child(responsibility),
          ),
      )
      .child(connection_cell(ctx, &connection_name, model, connection.is_some()))
      .child(
        words::note(&words::summary(
          ctx,
          profile.permissions(),
          connection.map(|c| c.provider()),
        ))
        .width(Dimension::Px(PERMISSIONS)),
      )
      .child(version_marker(ctx, entry))
      .child(words::note(&used).width(Dimension::Px(USED)))
      .child(icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted));
    opening(nav.to(Shown::Edit(id, AgentTab::default())))
      .id(format!("agent-{id}"))
      .width(Dimension::Pct(100.0))
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .child(row)
  }
}
pub fn header(ctx: &Ctx) -> Row {
  let headings = [
    (ColumnWidth::Fill, "agents.columns.profile"),
    (ColumnWidth::Fixed(CONNECTION), "agents.columns.connection"),
    (ColumnWidth::Fixed(PERMISSIONS), "agents.columns.permissions"),
    (ColumnWidth::Fixed(VERSION), "agents.columns.version"),
    (ColumnWidth::Fixed(USED), "agents.columns.used"),
  ]
  .map(|(width, key)| (width, ctx.t(key)));
  let headings: Vec<_> = headings.iter().map(|(width, text)| (*width, Some(&**text))).collect();
  table_header("agents-table-header", &headings, true)
}
fn version_marker(ctx: &mut Ctx, entry: &ProfileEntry) -> Row {
  let state = entry.state;
  let (glyph, color) = match state {
    VersionState::Published(_) => (Icon::CircleCheck, PaletteColor::Success),
    VersionState::StaleDraft { .. } => (Icon::CircleAlert, PaletteColor::Warning),
    VersionState::Draft { published: None } => (Icon::CircleDashed, PaletteColor::TextMuted),
    VersionState::Draft { .. } => (Icon::CircleDot, PaletteColor::TextSecondary),
  };
  Row::new()
    .width(Dimension::Px(VERSION))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, color.clone()))
    .child(
      Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(
        Truncated::new(
          format!("agent-version-{}", entry.id),
          words::list_version(ctx, state),
          crate::theme::ExtraTypography::Prose.role(),
        )
        .color(if matches!(state, VersionState::Published(_)) {
          PaletteColor::TextSecondary
        } else {
          color
        })
        .render(ctx),
      ),
    )
}
fn connection_cell(ctx: &Ctx, connection_name: &str, model: std::sync::Arc<str>, connected: bool) -> Column {
  Column::new()
    .width(Dimension::Px(CONNECTION))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(
          if connected { Icon::Plug } else { Icon::Unplug },
          IconSize::Sm,
          if connected {
            PaletteColor::TextMuted
          } else {
            PaletteColor::Warning
          },
        ))
        .child(
          Text::new(connection_name)
            .variant(TypographyStyle::Description)
            .color(if connected {
              PaletteColor::TextSecondary
            } else {
              PaletteColor::Warning
            })
            .nowrap(),
        ),
    )
    .child(
      Text::new(&if connected {
        model
      } else {
        ctx.t("agents.chooseConnection")
      })
      .variant(TypographyStyle::Caption)
      .nowrap(),
    )
}
