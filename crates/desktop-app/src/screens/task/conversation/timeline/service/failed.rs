//! A start that failed for a reason the person can act on, from the runner's `service_start_failed` note (`10 ·
//! States` `oXVuM`), in a `danger` stroke:
//!
//! - 10a, Docker not available (`Card · postgres can't start: Docker isn't running` `SGntT`): a `container` glyph
//!   (drawn as `database`: `lucide.ttf` maps no `container`), *postgres can't start: Docker isn't running* and
//!   `start_service postgres · 15:12`; what its per-run data needs and what to do, the services that wait for it; the
//!   macOS line under an `info` glyph (`Mac` `HlzNB`); then Retry and Edit.
//! - 10c, a readiness timeout (`Card · team-web wasn't ready in 90 s` `DPLN1`): a `timer-off` glyph, *team-web wasn't
//!   ready in 90 s* and what readiness asked; what Orchester asked and that it stopped the service, the services that
//!   keep running; its last lines with Orchester's own line last; then Restart, *Edit readiness* and *Open log*.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  service::{ReadyAsked, ServiceStartFailure, StartFailureCause},
  time::Timestamp,
};
use orchester_workspace::display;

use super::{super::Timeline, actions::LogPlace};
use crate::{
  theme::{IconSize, ServiceCardsText},
  ui::{
    icon::{Icon, icon},
    service_card::{Meta, TailLine, body, frame, head, log_tail},
  },
};

pub(super) fn card(
  ctx: &mut Ctx,
  timeline: &Timeline,
  id: &str,
  (failure, at): (&ServiceStartFailure, Timestamp),
) -> Element {
  match &failure.cause {
    StartFailureCause::DockerMissing { .. } => docker(ctx, timeline, id, (failure, at), true),
    StartFailureCause::DockerNotAnswering { .. } => docker(ctx, timeline, id, (failure, at), false),
    StartFailureCause::NotReady { secs, asked } => not_ready(ctx, timeline, id, failure, (*secs, asked)),
  }
}

/// 10a: Docker was `missing`, or there and not answering.
fn docker(
  ctx: &mut Ctx,
  timeline: &Timeline,
  id: &str,
  (failure, at): (&ServiceStartFailure, Timestamp),
  missing: bool,
) -> Element {
  let service = failure.service.as_str();
  let named = [("service", service.to_owned())];
  let (title, said) = match (missing, failure.not_started.is_empty()) {
    (true, true) => ("service_cards.docker.missingTitle", "service_cards.docker.missingBody"),
    (true, false) => (
      "service_cards.docker.missingTitle",
      "service_cards.docker.missingBodyWaiting",
    ),
    (false, true) => (
      "service_cards.docker.notRunningTitle",
      "service_cards.docker.notRunningBody",
    ),
    (false, false) => (
      "service_cards.docker.notRunningTitle",
      "service_cards.docker.notRunningBodyWaiting",
    ),
  };
  let said = ctx.t_args(
    said,
    [
      ("service", service.to_owned()),
      ("services", super::listed(ctx, &failure.not_started)),
    ],
  );
  let meta = ctx.t_args(
    "service_cards.docker.meta",
    [("service", service.to_owned()), ("time", display::clock(at))],
  );
  let mac = Row::new()
    .id(format!("{id}-mac"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Info, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("service_cards.docker.mac"))
        .variant(ServiceCardsText::Detail.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    );
  let labels = (
    ctx.t("service_cards.actions.retry"),
    ctx.t_args("service_cards.actions.editService", named),
  );
  let actions = super::actions(ctx, timeline, (id, service), (labels, LogPlace::None));
  frame(id, PaletteColor::Danger)
    .describe("service", service.to_owned())
    .describe(
      "cause",
      if missing {
        "docker_missing"
      } else {
        "docker_not_answering"
      },
    )
    .child(head(
      id,
      (Icon::Database, PaletteColor::Danger),
      &ctx.t_args(title, [("service", service.to_owned())]),
      (&meta, Meta::AfterTitle),
    ))
    .child(body(&format!("{id}-body"), &said))
    .child(mac)
    .with_children(actions)
    .into()
}

/// 10c: not ready within `secs`, asked as `asked` says.
fn not_ready(
  ctx: &mut Ctx,
  timeline: &Timeline,
  id: &str,
  failure: &ServiceStartFailure,
  (secs, asked): (u64, &ReadyAsked),
) -> Element {
  let service = failure.service.as_str();
  let (meta, said, last) = readiness_words(ctx, failure, (secs, asked));
  let said = ctx.t_args(said, words(ctx, failure, asked));
  let lines: Vec<TailLine> = failure
    .lines
    .iter()
    .map(|row| TailLine::of_row(service, row))
    .chain([TailLine {
      source: ctx.t("service_cards.orchester"),
      text: last,
      danger: true,
    }])
    .collect();
  let title = ctx.t_args(
    "service_cards.ready.title",
    [("service", service.to_owned()), ("seconds", secs.to_string())],
  );
  let labels = (
    ctx.t("service_cards.actions.restart"),
    ctx.t("service_cards.actions.editReadiness"),
  );
  let actions = super::actions(ctx, timeline, (id, service), (labels, LogPlace::Last));
  frame(id, PaletteColor::Danger)
    .describe("service", service.to_owned())
    .describe("cause", "not_ready")
    .child(head(
      id,
      (Icon::TimerOff, PaletteColor::Danger),
      &title,
      (&meta, Meta::AfterTitle),
    ))
    .child(body(&format!("{id}-body"), &said))
    .child(log_tail(&format!("{id}-lines"), &lines))
    .with_children(actions)
    .into()
}

/// What readiness asked, the key of what the body says of it, and Orchester's own last line, for a service not
/// ready within `secs`.
fn readiness_words(
  ctx: &Ctx,
  failure: &ServiceStartFailure,
  (secs, asked): (u64, &ReadyAsked),
) -> (Arc<str>, &'static str, Arc<str>) {
  let service = failure.service.as_str();
  let seconds = secs.to_string();
  // Nothing else of the step runs: the body names no services that keep running.
  let alone = failure.running.is_empty();
  match asked {
    ReadyAsked::Port { port } => (
      ctx.t_args("service_cards.ready.metaPort", [("port", port.to_string())]),
      if alone {
        "service_cards.ready.bodyPort"
      } else {
        "service_cards.ready.bodyPortRunning"
      },
      ctx.t_args(
        "service_cards.ready.lastPort",
        [
          ("port", port.to_string()),
          ("seconds", seconds),
          ("service", service.to_owned()),
        ],
      ),
    ),
    ReadyAsked::Http { path, .. } => (
      ctx.t_args("service_cards.ready.metaHttp", [("path", path.clone())]),
      if alone {
        "service_cards.ready.bodyHttp"
      } else {
        "service_cards.ready.bodyHttpRunning"
      },
      ctx.t_args(
        "service_cards.ready.lastHttp",
        [
          ("path", path.clone()),
          ("seconds", seconds),
          ("service", service.to_owned()),
        ],
      ),
    ),
    ReadyAsked::LogLine { pattern } => (
      ctx.t_args("service_cards.ready.metaLine", [("pattern", pattern.clone())]),
      if alone {
        "service_cards.ready.bodyLine"
      } else {
        "service_cards.ready.bodyLineRunning"
      },
      ctx.t_args(
        "service_cards.ready.lastLine",
        [("seconds", seconds), ("service", service.to_owned())],
      ),
    ),
  }
}

/// What a readiness timeout's body names: the service, what readiness asked, and the services that keep running.
fn words(ctx: &Ctx, failure: &ServiceStartFailure, asked: &ReadyAsked) -> [(&'static str, String); 3] {
  let asked = match asked {
    ReadyAsked::Port { port } => port.to_string(),
    ReadyAsked::Http { url, .. } => url.clone(),
    ReadyAsked::LogLine { pattern } => pattern.clone(),
  };
  [
    ("service", failure.service.clone()),
    ("asked", asked),
    ("services", super::listed(ctx, &failure.running)),
  ]
}
