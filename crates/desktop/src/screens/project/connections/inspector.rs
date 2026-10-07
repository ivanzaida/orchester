//! Selected connection details (`JuVaA` in `a4Lqb`, `pQ0DW` in `RZUKo`): status, provider,
//! what runs, where it runs, how it signs in, the last check, the models it offers and the
//! profiles that use it, in a details viewport with a scroll gutter (`xAfju`); the actions and
//! the status detail are pinned below it (`K9gvh`). A connection that needs attention says what
//! to do and offers checking again, as the expired-access state does (04.A). A native connection shows its endpoint,
//! key and models (`native_inspector`); this version cannot test it, so Test connection is unavailable and the status
//! detail says why.

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::layout_kind::ScrollState,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentConnection, CatalogModel, CatalogSource, ConnectionStatus},
  id::ResourceId,
  resource::ModelId,
  secret::Secret,
  time::Timestamp,
};
use orchester_exec::launch::Launch;

use super::{
  launch::launch_properties,
  native_inspector,
  status::{CHECKING, cli_too_old, status_detail_key, status_label, vendor_key},
};
use crate::{
  connections::data::ProfileUse,
  display,
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

/// Inspector width (`JuVaA`) and the longest executable path shown in full.
const INSPECTOR_WIDTH: f32 = 344.0;
const PATH_CHARS: usize = 36;

pub(super) type Handler = Arc<dyn Fn() + Send + Sync>;

pub struct InspectorActions {
  /// Checks the CLI again, or tests a native connection's model API through the runner; `None` while a
  /// check runs.
  pub check: Option<Handler>,
  pub edit: Option<Handler>,
  /// Opens the models of a native connection.
  pub edit_models: Option<Handler>,
  /// A link to a profile that uses it.
  pub profile: Arc<dyn Fn(ResourceId) -> Link + Send + Sync>,
  /// A link to the project's secrets, at a native connection's key secret.
  pub secrets: Arc<dyn Fn() -> Link + Send + Sync>,
}

pub struct InspectorState<'a> {
  pub uses: Vec<&'a ProfileUse>,
  pub checking: bool,
  /// Why the last check could not be stored.
  pub check_error: Option<Message>,
  /// What starts when the program runs.
  pub launch: Option<&'a Launch>,
  /// The project's secrets that are not archived, which a native connection's key must be among.
  pub secrets: &'a [Secret],
}

pub fn inspector(
  ctx: &Ctx,
  connection: &AgentConnection,
  (state, scroll): (&InspectorState, &ScrollState),
  actions: &InspectorActions,
) -> Element {
  let status = connection.status_with(state.secrets);
  let (status_icon, status_key) = if state.checking { CHECKING } else { status_label(status) };
  let details = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Step.role())
    .child(Text::new(connection.name().as_str()).variant(ExtraTypography::InspectorTitle.role()))
    .child(label(status_icon, &ctx.t(status_key)));
  let details = match (connection.api(), connection.executable()) {
    (Some(api), _) => details
      .with_children(native_inspector::properties(ctx, connection, api, state.secrets))
      .child(native_inspector::models(
        ctx,
        api,
        actions.edit_models.clone(),
        actions.check.clone(),
      )),
    (None, Some(executable)) => details
      .with_children(properties(ctx, connection, executable, state.launch))
      .child(models(ctx, connection))
      .child(button(
        ctx,
        "refresh-models",
        "connections.refreshModels",
        Icon::RefreshCw,
        actions.check.clone(),
      )),
    (None, None) => details,
  }
  .child(used_by(ctx, &state.uses, &actions.profile));
  let pinned = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_right(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Step.role())
    .with_children(native_inspector::status_actions(ctx, status, actions))
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
      actions.edit.clone(),
    ))
    .child(status_detail(ctx, connection, status, state.check_error.as_ref()));
  // The details scroll when they do not fit, their thumb in a gutter of its own (`NwsEg`) beside
  // the right padding; the actions and the status detail stay at the bottom, as in the GitHub
  // inspector.
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
        .with_scroll_state(scroll.clone())
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .scrollbar(gutter_scrollbar()),
    )
    .child(pinned)
    .into()
}

/// Provider, program file, what runs it and its script when it is a launcher, version,
/// execution, authentication and last check (`wAKbx`…`ouXB3`).
fn properties(ctx: &Ctx, connection: &AgentConnection, executable: &Path, launch: Option<&Launch>) -> Vec<Element> {
  let provider = connection.provider();
  let checked = connection.last_check().map_or_else(
    || ctx.t("connections.notCheckedYet"),
    |check| message_text(ctx, &display::elapsed(Timestamp::now(), check.at)),
  );
  let version = match (connection.version(), connection.last_check()) {
    (Some(version), _) => version.into(),
    (None, None) => ctx.t("connections.notCheckedYet"),
    (None, Some(_)) => ctx.t("github.connection.unknown"),
  };
  let mut rows = vec![
    property(&ctx.t("connections.property.provider"), &ctx.t(vendor_key(provider))),
    styled_property(
      &ctx.t("connections.property.executable"),
      &display::shorten_path(executable, PATH_CHARS),
      ValueStyle::Path,
    ),
  ];
  rows.extend(launch_properties(ctx, launch, PATH_CHARS));
  rows.extend([
    property(&ctx.t("connections.property.version"), &version),
    property(
      &ctx.t("connections.property.execution"),
      &ctx.t("connections.thisComputer"),
    ),
    property(
      &ctx.t("connections.property.authentication"),
      &ctx.t("connections.cliLogin"),
    ),
    property(&ctx.t("connections.property.lastChecked"), &checked),
  ]);
  rows
}

/// Models of the last check (`o5BqGF`, `JoYhX`) and where the list comes from.
fn models(ctx: &Ctx, connection: &AgentConnection) -> Element {
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
    CatalogSource::ProviderAndCurrentIds => "connections.modelSource.providerAndCurrentIds",
  };
  let too_old = connection
    .model_requirements()
    .iter()
    .filter_map(|(model, _)| ModelId::new(model).ok())
    .filter_map(|model| cli_too_old(ctx, connection, &model));
  column
    .child(model_chips(ctx, connection.models()))
    .child(Text::new(&ctx.t(source)).variant(ExtraTypography::InspectorNote.role()))
    .with_children(too_old.collect::<Vec<Element>>())
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
fn used_by(ctx: &Ctx, uses: &[&ProfileUse], profile: &Arc<dyn Fn(ResourceId) -> Link + Send + Sync>) -> Element {
  let links: Vec<Element> = uses
    .iter()
    .map(|used| {
      let text = ctx.t_args(
        "connections.usedByLink",
        [("role", &*used.role), ("model", used.model.as_str())],
      );
      ActionButton::new(text)
        .icon(Icon::ExternalLink)
        .fill_width()
        .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT))
        .link(profile(used.profile))
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

/// What the status means (`Thdx2`, `FhYJH`): how to sign in, or why the program did not answer.
fn status_detail(
  ctx: &Ctx,
  connection: &AgentConnection,
  status: ConnectionStatus,
  error: Option<&Message>,
) -> Element {
  let detail = match error {
    Some(error) => message_text(ctx, error),
    None => ctx.t(status_detail_key(status, connection.provider())),
  };
  Text::new(&detail)
    .variant(ExtraTypography::InspectorNote.role())
    .id("connection-status-detail")
    .into()
}

/// A full-width secondary button, disabled without a handler: while a check runs, or on a native
/// connection, whose status detail says why.
pub(super) fn button(ctx: &Ctx, id: &'static str, key: &str, glyph: Icon, handler: Option<Handler>) -> Element {
  let button = full_width(ctx, id, key, glyph);
  match handler {
    Some(handler) => button.on_click(move || handler()).render(ctx),
    None => button.on_click(|| {}).enabled(false).render(ctx),
  }
}

/// A full-width secondary button that opens `link`.
pub(super) fn link_button(ctx: &Ctx, id: &'static str, key: &str, glyph: Icon, link: Link) -> Element {
  full_width(ctx, id, key, glyph).link(link).render(ctx)
}

fn full_width(ctx: &Ctx, id: &'static str, key: &str, glyph: Icon) -> ActionButton {
  ActionButton::new(ctx.t(key))
    .icon(glyph)
    .id(id)
    .fill_width()
    .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT))
}
