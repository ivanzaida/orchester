//! Selected connection details (`ez6IT` in `a4Lqb`, `pQ0DW` in `RZUKo`): status, provider,
//! where it runs, how it signs in, the last check, the models it offers and the profiles that
//! use it. A connection that needs attention says what to do and offers checking again, as
//! the expired-access state does (04.A).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  connection::{CatalogModel, CatalogSource, Connection, ConnectionStatus},
  id::ResourceId,
  time::Timestamp,
};

use super::status::{CHECKING, status_detail_key, status_label, vendor_key};
use crate::{
  connections::data::ProfileUse,
  display,
  messages::Message,
  screens::project::page::TOPBAR_BUTTON_HEIGHT,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{button::ActionButton, icon::Icon, message_text, property::property, status_label::status_label as label},
};

/// Inspector width (`ez6IT`) and the longest executable path shown in full.
const INSPECTOR_WIDTH: f32 = 344.0;
const PATH_CHARS: usize = 36;

type Handler = Arc<dyn Fn() + Send + Sync>;

pub struct InspectorActions {
  pub check: Option<Handler>,
  pub edit: Handler,
  pub show_profile: Arc<dyn Fn(ResourceId) + Send + Sync>,
}

pub struct InspectorState<'a> {
  pub uses: Vec<&'a ProfileUse>,
  pub checking: bool,
  /// Why the last check could not be stored.
  pub check_error: Option<Message>,
}

pub fn inspector(ctx: &Ctx, connection: &Connection, state: &InspectorState, actions: &InspectorActions) -> Element {
  let status = connection.status();
  let (status_icon, status_key) = if state.checking { CHECKING } else { status_label(status) };
  Column::new()
    .id("connection-inspector")
    .width(Dimension::Px(INSPECTOR_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding(SpacingSize::Lg)
    .spacing(ExtraSpacing::Step.role())
    .background(PaletteColor::SurfacePanel)
    .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(connection.name().as_str()).variant(ExtraTypography::InspectorTitle.role()))
    .child(label(status_icon, &ctx.t(status_key)))
    .with_children(properties(ctx, connection))
    .child(models(ctx, connection))
    .child(button(
      ctx,
      "refresh-models",
      "connections.refreshModels",
      Icon::RefreshCw,
      actions.check.clone(),
    ))
    .child(used_by(ctx, &state.uses, &actions.show_profile))
    .child(Spacer::new().flex(1.0))
    .child(button(
      ctx,
      "test-connection",
      "connections.test",
      Icon::Activity,
      actions.check.clone(),
    ))
    .child(button(
      ctx,
      "edit-connection",
      "connections.edit",
      Icon::Settings,
      Some(actions.edit.clone()),
    ))
    .child(status_detail(ctx, connection, status, state.check_error.as_ref()))
    .into()
}

/// The design's properties (`CRVwa`…`X4Sz0`) plus the program file; the version is in the row.
fn properties(ctx: &Ctx, connection: &Connection) -> Vec<Element> {
  let provider = connection.provider();
  let checked = connection.last_check().map_or_else(
    || ctx.t("connections.notCheckedYet"),
    |check| message_text(ctx, &display::elapsed(Timestamp::now(), check.at)),
  );
  vec![
    property(&ctx.t("connections.property.provider"), &ctx.t(vendor_key(provider))),
    property(
      &ctx.t("connections.property.executable"),
      &display::shorten_path(connection.executable(), PATH_CHARS),
    ),
    property(
      &ctx.t("connections.property.execution"),
      &ctx.t("connections.thisComputer"),
    ),
    property(
      &ctx.t("connections.property.authentication"),
      &ctx.t("connections.cliLogin"),
    ),
    property(&ctx.t("connections.property.lastChecked"), &checked),
  ]
}

/// Models of the last check (`o5BqGF`, `JoYhX`) and where the list comes from.
fn models(ctx: &Ctx, connection: &Connection) -> Element {
  let column = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .child(Text::new(&ctx.t("connections.availableModels")).variant(TypographyStyle::Label));
  let Some(catalog) = connection.catalog() else {
    return column
      .child(Text::new(&ctx.t("connections.modelsAfterCheck")).variant(TypographyStyle::Caption))
      .into();
  };
  let source = match catalog.source() {
    CatalogSource::Provider => "connections.modelSource.provider",
    CatalogSource::DocumentedAliases => "connections.modelSource.aliases",
  };
  column
    .child(model_chips(ctx, connection.models()))
    .child(Text::new(&ctx.t(source)).variant(ExtraTypography::Footnote.role()))
    .into()
}

/// The models as status labels (`DeizF` with a sparkles icon), wrapping.
pub fn model_chips(ctx: &Ctx, models: &[CatalogModel]) -> Element {
  if models.is_empty() {
    return Text::new(&ctx.t("connections.noModelsListed"))
      .variant(TypographyStyle::Caption)
      .into();
  }
  Row::new()
    .width(Dimension::Pct(100.0))
    .wrap()
    .spacing(ExtraSpacing::Inline.role())
    .with_children(models.iter().map(|model| label(Icon::Sparkles, model.id().as_str())))
    .into()
}

/// Read-only links to the profiles on this connection (`zeEAI`, `Hxczz`).
fn used_by(ctx: &Ctx, uses: &[&ProfileUse], show: &Arc<dyn Fn(ResourceId) + Send + Sync>) -> Element {
  let links: Vec<Element> = uses
    .iter()
    .map(|used| {
      let (show, profile) = (show.clone(), used.profile);
      let text = ctx.t_args(
        "connections.usedByLink",
        [("role", &*used.role), ("model", &*used.model)],
      );
      ActionButton::new(text)
        .icon(Icon::ExternalLink)
        .fill_width()
        .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT))
        .on_click(move || show(profile))
        .render(ctx)
    })
    .collect();
  let column = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .child(Text::new(&ctx.t("connections.usedBy")).variant(TypographyStyle::Label));
  if links.is_empty() {
    return column
      .child(Text::new(&ctx.t("connections.notUsed")).variant(TypographyStyle::Caption))
      .into();
  }
  column.with_children(links).into()
}

/// What the status means (`Hzhjb`, `FhYJH`): how to sign in, or why the program did not answer.
fn status_detail(ctx: &Ctx, connection: &Connection, status: ConnectionStatus, error: Option<&Message>) -> Element {
  let detail = match error {
    Some(error) => message_text(ctx, error),
    None => ctx.t(status_detail_key(status, connection.provider())),
  };
  Text::new(&detail)
    .variant(ExtraTypography::Footnote.role())
    .id("connection-status-detail")
    .into()
}

/// A full-width secondary button, disabled without a handler (while a check runs).
fn button(ctx: &Ctx, id: &'static str, key: &str, glyph: Icon, handler: Option<Handler>) -> Element {
  let button = ActionButton::new(ctx.t(key))
    .icon(glyph)
    .id(id)
    .fill_width()
    .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT));
  match handler {
    Some(handler) => button.on_click(move || handler()).render(ctx),
    // A button without a handler would read as not available in this version; this one
    // only waits for the running check.
    None => button.on_click(|| {}).enabled(false).render(ctx),
  }
}
