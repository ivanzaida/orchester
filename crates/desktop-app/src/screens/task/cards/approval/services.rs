//! A start of services that waits for the person (`08 · Conversation · approve services` `hvQDS`, `Card · approve
//! services` `TienU`): one card for a service and what it needs, or a stack, in the waiting card's frame, its parts 10
//! apart. Its head is a `shield-alert` before *Start service team-web and what it needs: team-server, cloud-api,
//! postgres*, the agent · tool · time at its end; then that they start in this order and run these programs until the
//! step ends, which changed since they were approved and which never ran; the programs in start order on `code_bg` in a
//! `border` stroke, radius 6 (`Programs` `DbS50`), each row 8 by 12, 12 apart, under a `border` line but the first: its
//! number, its name in a column of 110, its program and arguments, and whether it is approved (`approved, unchanged`,
//! `changed: arguments` in `warning`, `first start` in `info`); the slots the start takes of the computer's *Services
//! at once* with a bar of them, 160 wide (`Slots line` `A3rtJZ`), the computer's free slots not known here; and the
//! answers (`Actions` `L4AMwQ`): *Approve*, *Approve for every run while unchanged* and *Deny*, 8 apart, then what Deny
//! tells the agent at the end.
//!
//! *Approve for every run while unchanged* makes a rule of the project for each service, so it confirms first in a
//! modal (confirmations are modals, the person, 2026-10-28) that only a person confirms; the card refuses it from
//! automation besides ([`super::super::decisions::Decisions::answer`]). The answers go to the run's runner as the
//! approval card's do.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  limits::ComputerServiceSlots,
  orchester_tools::{OrchesterTool, SERVER},
  run::PendingToolApproval,
  service::{ServiceApproval, started_by},
};
use orchester_workspace::display;

use super::{
  super::{Place, decisions::Given, frame::waiting_frame},
  answers::{Answering, held_line},
  request,
};
use crate::{
  queries::{self, Session, app},
  theme::{ExtraSpacing, ExtraTypography, RunServicesText, ServiceCardsText},
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::{Dialog, Tone, line, lines},
    icon::Icon,
    service_card::{Meta, Slot, body, head, slot_bar},
  },
};

/// A program's name column (`Name` `rX6C4`).
const NAME_WIDTH: f32 = 110.0;
/// The slot bar beside the slots line (`Bar` `kAreo`).
const BAR_WIDTH: f32 = 160.0;

/// The services a start of Orchester's `start_service` or `start_stack` asks to run, in start order; `None` for any
/// other request.
pub(super) fn asked(request: &PendingToolApproval) -> Option<Vec<ServiceApproval>> {
  let tool = OrchesterTool::from_name(&request.action)?;
  let starts = matches!(tool, OrchesterTool::StartService | OrchesterTool::StartStack);
  if request.command || request.server != SERVER || !starts {
    return None;
  }
  started_by(request.target.as_deref()?)
}

/// What the project's rules say of a service the start runs.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Standing {
  /// A rule approves it as it is now.
  Unchanged,
  /// A rule approved it before these parts of it changed.
  Changed(Vec<&'static str>),
  /// No rule approves it: it never ran here as approved for every run.
  First,
}

impl Standing {
  /// What the active rules `rules` say of `service`.
  fn of(service: &ServiceApproval, rules: &[ServiceApproval]) -> Self {
    let kept: Vec<&ServiceApproval> = rules.iter().filter(|rule| rule.name == service.name).collect();
    if kept.iter().any(|rule| rule.fingerprint == service.fingerprint) {
      return Self::Unchanged;
    }
    let Some(was) = kept.last() else {
      return Self::First;
    };
    let mut what = Vec::new();
    if was.program != service.program {
      what.push("service_cards.approve.what.program");
    }
    if was.arguments != service.arguments {
      what.push("service_cards.approve.what.arguments");
    }
    if was.variables != service.variables {
      what.push("service_cards.approve.what.variables");
    }
    if what.is_empty() {
      what.push("service_cards.approve.what.values");
    }
    Self::Changed(what)
  }
}

/// The card of a start of `services` at `place`, its answers `answering` gives; `confirming` holds the modal of
/// *Approve for every run while unchanged*, `failed` what became of an answer the runner refused.
pub(super) fn card(
  ctx: &mut Ctx,
  answering: &Answering<'_>,
  (place, services): (&Place, &[ServiceApproval]),
  confirming: &Signal<bool>,
  failed: Option<Element>,
) -> Element {
  let id = answering.id;
  let request = answering.request;
  let rules = answering.shown.map_or(&[][..], |shown| &shown.service_rules[..]);
  let standings: Vec<Standing> = services.iter().map(|service| Standing::of(service, rules)).collect();
  let meta = ctx.t_args(
    "service_cards.approve.meta",
    [
      ("agent", request.agent.clone()),
      ("tool", request.action.clone()),
      ("time", display::clock(request.requested_at)),
    ],
  );
  let title = title(ctx, request, services);
  let step = request::step(place, answering.shown).unwrap_or_default();
  let said = said(ctx, (services, &standings), &step);
  let padding = Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role());
  let head = head(
    id,
    (Icon::ShieldAlert, PaletteColor::Warning),
    &title,
    (&meta, Meta::AtEnd),
  )
  .into();
  waiting_frame(place, id.to_owned(), padding, || head)
    .describe("services", services.len().to_string())
    .spacing(ExtraSpacing::Loose.role())
    .child(body(&format!("{id}-body"), &said))
    .child(programs(ctx, id, (services, &standings)))
    .child(slots(ctx, id, services.len()))
    .child(answers(ctx, answering, confirming))
    .with_children(held_line(ctx, answering))
    .with_children(failed)
    .with_children(confirm(ctx, answering, services, confirming))
    .into()
}

/// *Start service web and what it needs: api, db*, the needs nearest first; a stack's start by its name.
fn title(ctx: &Ctx, request: &PendingToolApproval, services: &[ServiceApproval]) -> Arc<str> {
  let names: Vec<String> = services.iter().map(|service| service.name.clone()).collect();
  if request.action == OrchesterTool::StartStack.as_str() {
    let stack = request
      .scope
      .as_deref()
      .and_then(|arguments| serde_json::from_str::<serde_json::Value>(arguments).ok())
      .and_then(|arguments| arguments["name"].as_str().map(str::to_owned))
      .unwrap_or_default();
    return ctx.t_args(
      "service_cards.approve.titleStack",
      [("stack", stack), ("services", listed(ctx, &names))],
    );
  }
  let Some((service, needs)) = names.split_last() else {
    return ctx.t_args("service_cards.approve.title", [("service", String::new())]);
  };
  if needs.is_empty() {
    return ctx.t_args("service_cards.approve.title", [("service", service.clone())]);
  }
  let nearest: Vec<String> = needs.iter().rev().cloned().collect();
  ctx.t_args(
    "service_cards.approve.titleNeeds",
    [("service", service.clone()), ("needs", listed(ctx, &nearest))],
  )
}

/// That they run until `step` ends, which changed since they were approved and which never ran.
fn said(ctx: &Ctx, (services, standings): (&[ServiceApproval], &[Standing]), step: &str) -> String {
  let named = |want: fn(&Standing) -> bool| -> Vec<String> {
    services
      .iter()
      .zip(standings)
      .filter(|(_, standing)| want(standing))
      .map(|(service, _)| service.name.clone())
      .collect()
  };
  let changed = named(|standing| matches!(standing, Standing::Changed(_)));
  let first = named(|standing| *standing == Standing::First);
  let key = if services.len() == 1 {
    "service_cards.approve.bodyOne"
  } else {
    "service_cards.approve.body"
  };
  let mut said = vec![ctx.t_args(key, [("step", step.to_owned())])];
  if !changed.is_empty() {
    let key = if changed.len() == 1 {
      "service_cards.approve.changedOne"
    } else {
      "service_cards.approve.changed"
    };
    said.push(ctx.t_args(key, [("services", listed(ctx, &changed))]));
  }
  if !first.is_empty() {
    said.push(ctx.t_args("service_cards.approve.firstStart", [("services", listed(ctx, &first))]));
  }
  said.join(" ")
}

/// `names` as one list, as the conversation lists names.
fn listed(ctx: &Ctx, names: &[String]) -> String {
  names.join(&*ctx.t("conversation.list.separator"))
}

/// The programs in start order.
fn programs(ctx: &Ctx, id: &str, (services, standings): (&[ServiceApproval], &[Standing])) -> Element {
  let rows = services
    .iter()
    .zip(standings)
    .enumerate()
    .map(|(at, (service, standing))| {
      let (words, color) = match standing {
        Standing::Unchanged => (ctx.t("service_cards.approve.unchanged"), PaletteColor::TextMuted),
        Standing::Changed(what) => {
          let what: Vec<String> = what.iter().map(|key| ctx.t(key).to_string()).collect();
          (
            ctx.t_args("service_cards.approve.changedWhat", [("what", what.join(", "))]),
            PaletteColor::Warning,
          )
        }
        Standing::First => (ctx.t("service_cards.approve.first"), PaletteColor::Info),
      };
      let row = Row::new()
        .id(format!("{id}-program-{}", service.name))
        .describe("standing", words.to_string())
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
        .spacing(SpacingSize::Md)
        .align_items(Alignment::Center)
        .child(Text::new(&(at + 1).to_string()).variant(ExtraTypography::MonoNote.role()))
        .child(
          Row::new()
            .width(Dimension::Px(NAME_WIDTH))
            .flex_shrink(0.0)
            .child(Text::new(&service.name).variant(RunServicesText::Name.role()).nowrap()),
        )
        .child(
          Text::new(&service.command_line())
            .id(format!("{id}-program-{}-command", service.name))
            .variant(ServiceCardsText::Command.role())
            .selectable(true)
            .flex(1.0)
            .min_width(Dimension::Px(0.0)),
        )
        .child(
          Text::new(&words)
            .variant(ServiceCardsText::Detail.role())
            .color(color)
            .nowrap(),
        );
      if at == 0 {
        row
      } else {
        row.border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
      }
    });
  Column::new()
    .id(format!("{id}-programs"))
    .width(Dimension::Pct(100.0))
    .background(crate::theme::ExtraColor::CodeBackground.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(lurq::app::theme::RadiusSize::Md)
    .clip()
    .with_children(rows)
    .into()
}

/// The slots the start takes of the computer's *Services at once*, and a bar of them; without the bar until the
/// computer's limit is read.
fn slots(ctx: &mut Ctx, id: &str, count: usize) -> Element {
  let session = Session::of(ctx);
  let limit: Option<ComputerServiceSlots> = queries::state(&ctx.query(app::service_slots(session))).data;
  let Some(limit) = limit.map(|limit| limit.get().get()) else {
    return Row::new().id(format!("{id}-slots")).into();
  };
  let taken = u32::try_from(count).unwrap_or(u32::MAX);
  let bar: Vec<Slot> = (0..limit.max(taken))
    .map(|at| if at < taken { Slot::Taken } else { Slot::Free })
    .collect();
  Row::new()
    .id(format!("{id}-slots"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t_args(
        "service_cards.approve.slots",
        [("count", count.to_string()), ("limit", limit.to_string())],
      ))
      .id(format!("{id}-slots-words"))
      .variant(ServiceCardsText::Detail.role())
      .nowrap(),
    )
    .child(slot_bar(&format!("{id}-slots-bar"), &bar, Dimension::Px(BAR_WIDTH)))
    .into()
}

/// *Approve*, *Approve for every run while unchanged*, which opens its modal (`confirming`), and *Deny*, then what Deny
/// tells the agent.
fn answers(ctx: &Ctx, answering: &Answering<'_>, confirming: &Signal<bool>) -> Element {
  let id = answering.id;
  let approve = answering
    .button(
      ctx,
      Given::ApproveOnce,
      (ButtonKind::Primary, Icon::Check, "service_cards.approve.approve"),
    )
    .element_ref(answering.once.clone());
  let every_run = ActionButton::new(
    format!("{id}-every-run"),
    ButtonKind::Secondary,
    ctx.t("service_cards.approve.everyRun"),
  )
  .icon(Icon::ShieldCheck);
  let every_run = match answering.hold(Given::ApproveProject) {
    Err(true) => every_run.loading(),
    Err(false) => every_run.disabled(),
    Ok(_) => {
      let confirming = confirming.clone();
      every_run.on_click(move || confirming.set(true))
    }
  };
  let deny = answering.button(
    ctx,
    Given::Deny,
    (ButtonKind::DangerGhost, Icon::X, "service_cards.approve.deny"),
  );
  Row::new()
    .id(format!("{id}-answers"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(approve.in_form())
    .child(every_run.in_form())
    .child(deny.in_form())
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&ctx.t("service_cards.approve.denyTells"))
        .id(format!("{id}-deny-tells"))
        .variant(ExtraTypography::Note.role())
        .nowrap(),
    )
    .into()
}

/// The modal that confirms *Approve for every run while unchanged* while `confirming` holds and the answer can be
/// sent: what it covers and that a change asks again; only a person confirms it. *Cancel*, Esc and the scrim close it.
fn confirm(
  ctx: &Ctx,
  answering: &Answering<'_>,
  services: &[ServiceApproval],
  confirming: &Signal<bool>,
) -> Option<Element> {
  let open = answering.hold(Given::ApproveProject).ok()?;
  let id = format!("{}-every-run-confirm", answering.id);
  let names: Vec<String> = services.iter().map(|service| service.name.clone()).collect();
  let press = answering.press(Given::ApproveProject, open);
  let shut = confirming.clone();
  let confirm = ActionButton::new(
    format!("{id}-confirm"),
    ButtonKind::Primary,
    ctx.t("service_cards.approve.confirm.confirm"),
  )
  .icon(Icon::ShieldCheck)
  .on_click(move || {
    shut.set(false);
    press();
  });
  let covers = ctx.t_args(
    "service_cards.approve.confirm.covers",
    [("services", listed(ctx, &names))],
  );
  Some(
    Dialog::new(
      &id,
      Icon::ShieldCheck,
      ctx.t("service_cards.approve.confirm.title"),
      ctx.t("service_cards.approve.confirm.cancel"),
    )
    .tone(Tone::Warning)
    .what(ctx.t("service_cards.approve.confirm.what"))
    .part(lines(
      &format!("{id}-lines"),
      [
        line(format!("{id}-covers"), Icon::Layers, &covers),
        line(
          format!("{id}-asks-again"),
          Icon::RotateCcw,
          &ctx.t("service_cards.approve.confirm.asksAgain"),
        ),
      ],
    ))
    .person_only(ctx.t("service_cards.approve.confirm.personOnly"))
    .action(confirm)
    .render(ctx, confirming),
  )
}
