//! Agent list master NA7sg: 60 high, with mCvPL's five columns.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::agents::{
  data::{ProfileEntry, ProjectAgents},
  form::stored_effort,
};

use super::{View, words};
use crate::{
  theme::{ExtraColor, ExtraSpacing, IconSize},
  ui::{
    avatar::profile_avatar,
    icon::{Icon, icon},
    pressable::pressable,
    table::{ColumnWidth, table_header},
  },
};

pub const ROW_HEIGHT: f32 = 60.0;
pub const TABLE_CHROME: f32 = 70.0;
const CONNECTION: f32 = 190.0;
const PERMISSIONS: f32 = 240.0;
const VERSION: f32 = 170.0;
const USED: f32 = 120.0;

#[derive(Clone)]
pub struct RowProps {
  pub entry: ProfileEntry,
  pub listed: ProjectAgents,
  pub view: Signal<View>,
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
    let view = props.view;
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
          .child(profile_avatar(&format!("agent-avatar-{id}"), profile.role().as_str()))
          .child(
            Column::new()
              .flex(1.0)
              .spacing(ExtraSpacing::Tight.role())
              .child(Text::new(profile.role().as_str()).variant(TypographyStyle::Label))
              .child(Text::new(profile.responsibilities()).variant(TypographyStyle::Description)),
          ),
      )
      .child(
        Column::new()
          .width(Dimension::Px(CONNECTION))
          .spacing(ExtraSpacing::Tight.role())
          .child(Text::new(&connection_name).variant(TypographyStyle::Description))
          .child(Text::new(&model).variant(TypographyStyle::Caption)),
      )
      .child(
        words::note(&words::summary(
          ctx,
          profile.permissions(),
          connection.map(|c| c.provider()),
        ))
        .width(Dimension::Px(PERMISSIONS)),
      )
      .child(words::note(&words::list_version(ctx, entry.state)).width(Dimension::Px(VERSION)))
      .child(words::note(&used).width(Dimension::Px(USED)))
      .child(icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted));
    pressable(move || view.set(View::Edit(id)))
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
