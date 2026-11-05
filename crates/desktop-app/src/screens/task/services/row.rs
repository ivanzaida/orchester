//! One service's row (`postgres` `qWnt1` … `team-web` `T0YEZ` in `Rows` `J03p0s`): 10 over and under, 14 at its sides
//! and between its cells, a `border` line over every row but the first, the selected one on `hover`. Its cells: the
//! state's badge (92), the name in Geist Mono 12.5/500 with its UI driver's chip (200), the port (64), the address
//! (the rest: a URL that opens in the in-app Browser tab while it is ready, what a starting one waits for, why one
//! that does not run stopped, in `danger` for a crash), the uptime (76, at its end), and the actions (178, at its end):
//! Restart and Stop while it is ready, Stop while it starts, Restart and the log's chevron once it crashed, Start once
//! it stopped. A press on the row shows its log under it; another hides it.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, TypographyStyle},
  },
  components::{Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::service::{Service, UiDriver};
use orchester_ipc::services::{RunService, RunServiceState};

use super::{
  actions::ServiceRequest,
  since::{Counted, Since, SinceProps},
  words,
};
use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, RunServicesText},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::{BadgeTone, TagChip, badge},
    icon::{Icon, icon},
    icon_button::IconButton,
    pressable::pressable,
  },
};

/// The cells' widths (`State` `QEOFY`, `Name` `ApBxD`, `Port` `G19ttw`, `Uptime` `K6Swgg`, `Actions` `jzRK3`); the
/// address takes the rest.
pub(super) const STATE_WIDTH: f32 = 92.0;
pub(super) const NAME_WIDTH: f32 = 200.0;
pub(super) const PORT_WIDTH: f32 = 64.0;
pub(super) const UPTIME_WIDTH: f32 = 76.0;
pub(super) const ACTIONS_WIDTH: f32 = 178.0;

/// What a row shows besides its service.
pub(super) struct RowParts<'a> {
  pub id: Arc<str>,
  pub definition: Option<Arc<Service>>,
  /// The first row has no line over it.
  pub first: bool,
  pub selected: bool,
  /// The request under way about it, if one is.
  pub pending: Option<&'a ServiceRequest>,
  /// Runs a request about it.
  pub request: Arc<dyn Fn(ServiceRequest) + Send + Sync>,
  /// Shows or hides its log.
  pub select: Arc<dyn Fn() + Send + Sync>,
  /// Opens an address in the in-app Browser tab.
  pub open: Arc<dyn Fn(Arc<str>) + Send + Sync>,
}

/// The row of `service`.
pub(super) fn row(ctx: &mut Ctx, service: &RunService, parts: &RowParts<'_>) -> Element {
  let id = parts.id.clone();
  let select = parts.select.clone();
  let mut row = pressable(move || select())
    .id(id.to_string())
    .describe("service", service.name.clone())
    .describe("state", words::state_key(service.state))
    .describe("selected", if parts.selected { "true" } else { "false" })
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      ExtraSpacing::Inset.role(),
      ExtraSpacing::Loose.role(),
    ))
    .spacing(ExtraSpacing::Inset.role())
    .align_items(Alignment::Center)
    .child(cell(STATE_WIDTH).child(state_badge(ctx, &id, service.state)))
    .child(name(ctx, &id, service, parts.definition.as_deref()))
    .child(
      cell(PORT_WIDTH).child(
        Text::new(&words::port(ctx, service.port))
          .id(format!("{id}-port"))
          .variant(RunServicesText::Port.role())
          .nowrap(),
      ),
    )
    .child(address(ctx, &id, service, parts))
    .child(uptime(ctx, &id, service))
    .child(actions(ctx, &id, service, parts));
  if !parts.first {
    row = row.border_top(Border::inside(BorderSize::Sm, PaletteColor::Border));
  }
  if parts.selected {
    row.background(ExtraColor::Hover.role()).into()
  } else {
    row
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .into()
  }
}

fn cell(width: f32) -> Row {
  Row::new()
    .width(Dimension::Px(width))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
}

/// The state's badge: Ready in `success`, Starting in `info`, Crashed in `danger`, Stopped neutral.
fn state_badge(ctx: &Ctx, id: &str, state: RunServiceState) -> Row {
  let tone = match state {
    RunServiceState::Ready => BadgeTone::Success,
    RunServiceState::Starting => BadgeTone::Info,
    RunServiceState::Crashed { .. } => BadgeTone::Danger,
    RunServiceState::Stopped => BadgeTone::Neutral,
  };
  badge(&format!("{id}-state"), tone, &ctx.t(words::state_key(state)))
}

/// The name and its UI driver's chip.
fn name(ctx: &Ctx, id: &str, service: &RunService, definition: Option<&Service>) -> Row {
  let driver = definition.map_or(UiDriver::None, Service::ui_driver);
  cell(NAME_WIDTH)
    .spacing(ExtraSpacing::Snug.role())
    .child(
      Text::new(&service.name)
        .id(format!("{id}-name"))
        .variant(RunServicesText::Name.role())
        .nowrap(),
    )
    .child(TagChip::new(format!("{id}-driver"), words::driver(ctx, driver)))
}

/// Where the service listens, what it waits for, or why it stopped.
fn address(ctx: &mut Ctx, id: &str, service: &RunService, parts: &RowParts<'_>) -> Element {
  let room = Row::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center);
  match service.state {
    RunServiceState::Ready if service.url.starts_with("http") => {
      room.child(link(ctx, id, &service.url, parts.open.clone())).into()
    }
    RunServiceState::Ready => room
      .child(
        Text::new(&service.url)
          .id(format!("{id}-address"))
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted)
          .nowrap(),
      )
      .into(),
    RunServiceState::Starting => {
      let props = SinceProps {
        id: format!("{id}-address").into(),
        service: service.clone(),
        counted: Counted::Waiting(parts.definition.clone()),
      };
      room
        .child(ctx.mount_keyed::<Since>(&format!("{id}-waiting"), props))
        .into()
    }
    RunServiceState::Crashed { .. } | RunServiceState::Stopped => {
      let color = if matches!(service.state, RunServiceState::Crashed { .. }) {
        PaletteColor::Danger
      } else {
        PaletteColor::TextMuted
      };
      room
        .child(
          Text::new(&words::ended(ctx, service))
            .id(format!("{id}-address"))
            .variant(TypographyStyle::Description)
            .color(color)
            .nowrap(),
        )
        .into()
    }
  }
}

/// A ready service's URL: a 12 `text_secondary` globe, the URL in Geist Mono 12 and a 12 `text_muted` arrow (`p3yTp`);
/// a press opens it in the in-app Browser tab.
fn link(ctx: &Ctx, id: &str, url: &str, open: Arc<dyn Fn(Arc<str>) + Send + Sync>) -> Element {
  let address: Arc<str> = url.into();
  let opened = address.clone();
  pressable(move || open(opened.clone()))
    .id(format!("{id}-address"))
    .describe(
      "a11y_name",
      ctx.t_args("run_services.actions.open", [("url", url.to_owned())]),
    )
    .spacing(ExtraSpacing::Snug.role())
    .child(icon(Icon::Globe, IconSize::Sm, PaletteColor::TextSecondary))
    .child(
      Text::new(&address)
        .id(format!("{id}-url"))
        .variant(ExtraTypography::PropertyMono.role())
        .nowrap(),
    )
    .child(icon(Icon::ArrowUpRight, IconSize::Sm, PaletteColor::TextMuted))
    .into()
}

/// How long it has run while it runs, `—` once it does not.
fn uptime(ctx: &mut Ctx, id: &str, service: &RunService) -> Row {
  let shown: Element = if matches!(service.state, RunServiceState::Ready | RunServiceState::Starting) {
    let props = SinceProps {
      id: format!("{id}-uptime").into(),
      service: service.clone(),
      counted: Counted::Uptime,
    };
    ctx.mount_keyed::<Since>(&format!("{id}-ticking"), props)
  } else {
    Text::new(&ctx.t("run_services.uptime.none"))
      .id(format!("{id}-uptime"))
      .variant(RunServicesText::Uptime.role())
      .nowrap()
      .into()
  };
  cell(UPTIME_WIDTH).justify(Justify::End).child(shown)
}

/// What the person may do with it now.
fn actions(ctx: &mut Ctx, id: &str, service: &RunService, parts: &RowParts<'_>) -> Row {
  let name: Arc<str> = service.name.as_str().into();
  let button = |(suffix, label): (&str, Arc<str>), kind: ButtonKind, glyph: Icon, request: ServiceRequest| {
    let under_way = parts.pending.is_some_and(|pending| *pending == request);
    let request_it = parts.request.clone();
    let button = ActionButton::new(format!("{id}-{suffix}"), kind, label)
      .icon(glyph)
      .small();
    if under_way {
      button.loading()
    } else if parts.pending.is_some() {
      button.disabled()
    } else {
      button.on_click(move || request_it(request.clone()))
    }
  };
  let restart = ("restart", ctx.t("run_services.actions.restart"));
  let stop = ("stop", ctx.t("run_services.actions.stop"));
  let start = ("start", ctx.t("run_services.actions.start"));
  let restart = button(
    restart,
    ButtonKind::Secondary,
    Icon::RotateCw,
    ServiceRequest::Restart(name.clone()),
  );
  let stop = button(
    stop,
    ButtonKind::Ghost,
    Icon::Square,
    ServiceRequest::Stop(name.clone()),
  );
  let shown: Vec<Element> = match service.state {
    RunServiceState::Ready => vec![restart.into(), stop.into()],
    RunServiceState::Starting => vec![stop.into()],
    RunServiceState::Crashed { .. } => vec![restart.into(), chevron(ctx, id, service, parts)],
    RunServiceState::Stopped => {
      vec![button(start, ButtonKind::Secondary, Icon::Play, ServiceRequest::Restart(name)).into()]
    }
  };
  cell(ACTIONS_WIDTH)
    .justify(Justify::End)
    .spacing(ExtraSpacing::Snug.role())
    .with_children(shown)
}

/// The chevron that shows or hides the log of a service that crashed (`Collapse` `wOZXQ`).
fn chevron(ctx: &mut Ctx, id: &str, service: &RunService, parts: &RowParts<'_>) -> Element {
  let (glyph, key) = if parts.selected {
    (Icon::ChevronUp, "run_services.actions.hideLog")
  } else {
    (Icon::ChevronDown, "run_services.actions.showLog")
  };
  let select = parts.select.clone();
  IconButton::new(
    format!("{id}-log"),
    glyph,
    ctx.t_args(key, [("name", service.name.clone())]),
  )
  .small()
  .on_click(move || select())
  .render(ctx)
}
