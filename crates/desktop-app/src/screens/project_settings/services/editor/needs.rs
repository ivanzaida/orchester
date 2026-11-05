//! Needs (`Needs` `J8FFD`): the services it needs as chips with their ×, *Add a service* offering the project's other
//! services (`Pills` `BKuHO`), the chain a start brings them up in, numbered, the service itself last and marked
//! (`Start order` `Dybbp`), and that it is resolved from each service's needs (`Note` `QFm3u`). Beside the pane the
//! *Start order* card lists the same chain with how each tells it is ready (`Start order` `Sow5a`). A need added or
//! removed saves at once. Needs that do not resolve (a cycle, a service gone) say so where the chain would be.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, SelectOption, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::agent_testing::editor::order::{Ready, Started, start_order};

use super::{
  super::super::kit::{FormField, Group},
  Commit, LABEL_WIDTH, View,
  fields::Fields,
};
use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, ServicesText, SettingsText},
  ui::{
    chip::TagChip,
    field::{SelectField, field_error},
    icon::{Icon, icon},
    message::message_text,
  },
};

/// *Add a service* (`Select` `L6b60x`).
const ADD_WIDTH: f32 = 150.0;
/// A chain chip (`postgres` `R3grk`).
const CHAIN_HEIGHT: f32 = 26.0;
/// A start order row's round mark (`Num` `p5XMfp`).
const MARK_SIZE: f32 = 18.0;

/// The Needs group.
pub(super) fn needs_group(ctx: &mut Ctx, fields: &Fields, view: &View, commit: &Commit) -> Element {
  let needs = fields.needs.get().0;
  let error = view.problems().and_then(|problems| problems.needs.clone());
  let order = view
    .built
    .as_ref()
    .ok()
    .map(|service| start_order(service, view.services()));
  let pills = Row::new()
    .id("settings-service-needs")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .with_children(needs.iter().map(|need| pill(ctx, fields, need, commit)))
    .child(add(ctx, fields, view));
  let mut control = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(pills);
  control = match (error, order) {
    (Some(error), _) | (None, Some(Err(error))) => {
      control.child(field_error("settings-service-needs-error", &message_text(ctx, &error)))
    }
    (None, Some(Ok(order))) => control.child(chain(&order)).child(
      Text::new(&ctx.t("project_settings.services.editor.orderNote"))
        .id("settings-service-order-note")
        .variant(ExtraTypography::Note.role()),
    ),
    (None, None) => control,
  };
  let row = FormField::new(
    "settings-service-needs",
    ctx.t("project_settings.services.editor.needs"),
  )
  .hint(ctx.t("project_settings.services.editor.needsHint"))
  .label_width(LABEL_WIDTH)
  .render(ctx, control);
  Group::new(
    "settings-service-needs-group",
    ctx.t("project_settings.services.editor.needs"),
  )
  .row(row)
  .into()
}

/// A need's chip; its × removes it and saves.
fn pill(ctx: &Ctx, fields: &Fields, need: &str, commit: &Commit) -> Element {
  let (needs, commit, removed) = (fields.needs.clone(), commit.clone(), need.to_owned());
  TagChip::new(format!("settings-service-need-{need}"), need.to_owned())
    .mono()
    .pill()
    .removable(
      ctx.t_args("project_settings.services.editor.removeNeed", [("name", need)]),
      move || {
        needs.update(|needs| needs.0.retain(|need| *need != removed));
        commit();
      },
    )
    .into()
}

/// *Add a service*: the project's other services not needed yet; choosing one adds it after the others and saves
/// (the page follows [`Fields::adding`]).
fn add(ctx: &Ctx, fields: &Fields, view: &View) -> Element {
  let needs = fields.needs.get().0;
  let typed = fields.name.get();
  let own = view.base().map_or(typed.trim(), |base| base.name().as_str()).to_owned();
  let offered: Vec<SelectOption<String>> = view
    .services()
    .iter()
    .map(|entry| entry.name.to_string())
    .filter(|name| *name != own && !needs.contains(name))
    .map(|name| SelectOption::new(name.clone(), name))
    .collect();
  let placeholder = if offered.is_empty() {
    "project_settings.services.editor.noOtherServices"
  } else {
    "project_settings.services.editor.addNeed"
  };
  let select = SelectField::new(
    "settings-service-add-need",
    ctx.t("project_settings.services.editor.addNeed"),
    fields.adding.clone(),
    offered.clone(),
  )
  .placeholder(ctx.t(placeholder))
  .width(Dimension::Px(ADD_WIDTH));
  if offered.is_empty() { select.locked() } else { select }.render(ctx)
}

/// The chain a start brings the services up in: each chip numbered, an arrow between them, the service itself last
/// on the `hover` fill in the `border_focus` stroke (`team-web` `uvhXz`).
fn chain(order: &[Started]) -> Element {
  let mut row = Row::new()
    .id("settings-service-chain")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center);
  for (at, started) in order.iter().enumerate() {
    if at > 0 {
      row = row.child(icon(Icon::ArrowRight, IconSize::Sm, PaletteColor::TextMuted));
    }
    let (fill, stroke, name) = if started.this {
      (
        ExtraColor::Hover.role(),
        PaletteColor::BorderFocus,
        PaletteColor::TextPrimary,
      )
    } else {
      (
        PaletteColor::SurfaceRaised,
        PaletteColor::Border,
        PaletteColor::TextSecondary,
      )
    };
    row = row.child(
      Row::new()
        .id(format!("settings-service-chain-{}", started.name))
        .height(Dimension::Px(CHAIN_HEIGHT))
        .padding(Padding::horizontal(SpacingSize::Sm))
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .background(fill)
        .border(Border::inside(BorderSize::Sm, stroke))
        .rounded(RadiusSize::Md)
        .child(Text::new(&(at + 1).to_string()).variant(ServicesText::ChainNumber.role()))
        .child(
          Text::new(&started.name)
            .variant(ServicesText::ChainName.role())
            .color(name),
        ),
    );
  }
  row.into()
}

/// The order a start brings the service up in, for the side column; `None` while the fields make no service or the
/// needs do not resolve, which the Needs group says.
fn resolved(view: &View) -> Option<Vec<Started>> {
  view
    .built
    .as_ref()
    .ok()
    .and_then(|service| start_order(service, view.services()).ok())
}

/// The *Start order* card (`Sow5a`): its title, then a row per service, numbered in a round mark (the service itself on
/// `accent`), its name and how it tells it is ready.
pub(super) fn start_order_card(ctx: &Ctx, view: &View) -> Option<Element> {
  let order = resolved(view)?;
  let rows = order
    .iter()
    .enumerate()
    .map(|(at, started)| order_row(ctx, at, started));
  Some(
    Column::new()
      .id("settings-service-start-order")
      .width(Dimension::Pct(100.0))
      .padding(Padding::all(SpacingSize::Md))
      .spacing(ExtraSpacing::Loose.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .child(
        Text::new(&ctx.t("project_settings.services.editor.startOrder"))
          .id("settings-service-start-order-title")
          .variant(SettingsText::SectionTitle.role()),
      )
      .with_children(rows)
      .into(),
  )
}

fn order_row(ctx: &Ctx, at: usize, started: &Started) -> Element {
  let (fill, number, name) = if started.this {
    (
      PaletteColor::Accent,
      ExtraColor::OnAccent.role(),
      PaletteColor::TextPrimary,
    )
  } else {
    (
      PaletteColor::SurfaceRaised,
      PaletteColor::TextSecondary,
      PaletteColor::TextSecondary,
    )
  };
  let ready = match &started.ready {
    Ready::PortOpen => ctx.t("project_settings.services.editor.ready.port"),
    Ready::Http(path) => ctx.t_args("project_settings.services.editor.ready.http", [("path", &**path)]),
    Ready::LogLine => ctx.t("project_settings.services.editor.ready.log"),
  };
  let id = format!("settings-service-order-{}", started.name);
  Row::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Row::new()
        .width(Dimension::Px(MARK_SIZE))
        .height(Dimension::Px(MARK_SIZE))
        .flex_shrink(0.0)
        .align_items(Alignment::Center)
        .justify(Justify::Center)
        .background(fill)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(ExtraRadius::Full.role())
        .child(
          Text::new(&(at + 1).to_string())
            .variant(ServicesText::OrderNumber.role())
            .color(number),
        ),
    )
    .child(
      Text::new(&started.name)
        .id(format!("{id}-name"))
        .variant(ServicesText::OrderName.role())
        .color(name),
    )
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&ready)
        .id(format!("{id}-ready"))
        .variant(ExtraTypography::Note.role()),
    )
    .into()
}
