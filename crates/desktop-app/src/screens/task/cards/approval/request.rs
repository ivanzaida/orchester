//! What a waiting tool request asks (`Approval card` `gwOaJ`, with the agent's note of `YDJsC`): *Approve
//! command?* and since when it waits; what the agent wants and what waiting does; its category chip and the
//! setting it asks under; why the classifier asks; that auto mode is reviewing it, or why it did not (the hard
//! rule that held it, a note as *reviewing* is: no frame draws it); the command whole, or an MCP call's
//! arguments, in a mono box with Copy; the agent · step · run · task, the working directory and the agent's
//! note; and the words Deny tells the agent. A request that came to the person by the run's approval mode says
//! less: what it asks on one line and why it came ([`came_body`], 04c `vZcoN`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::run::{
  Answerer, CameToPerson, NativeCommandAsk, PendingToolApproval, PersonReason, denial_message,
};
use orchester_workspace::{display, tasks::approvals::ShownApproval};

use super::{
  super::{
    Place,
    frame::{head as card_head, note},
  },
  came, could_not_run,
  decided::asked,
  words,
};
use crate::{
  theme::{CardsText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    chip::TagChip,
    code_box::CodeBox,
    detail_row::{DetailValue, detail_row},
    icon::Icon,
    message::message_text,
  },
};

/// "03 Implement" as the chat names the step the agent asks in (`zZoFw` in `YDJsC`), else its name.
pub(super) fn step(place: &Place, shown: Option<&ShownApproval>) -> Option<Arc<str>> {
  place
    .step
    .clone()
    .or_else(|| shown.and_then(|shown| shown.step.clone()))
}

/// Between the head and the answers of a request that `came` to the person (`vO7CW`): what it asks on one mono
/// line (`Body` `ZNwbH`), then why it came ([`came::line`]; why the reviewer could not run, [`could_not_run::line`]);
/// in the Needs you panel, whose item says what it asks, only why. A note says when auto mode's reviewer judges it
/// all the same.
pub(super) fn came_body(
  ctx: &mut Ctx,
  id: &str,
  (place, request, shown): (&Place, &PendingToolApproval, Option<&ShownApproval>),
  (came, reviewing): (&CameToPerson, bool),
) -> Vec<Element> {
  let action = (!place.in_panel).then(|| {
    Text::new(&asked(ctx, request))
      .id(format!("{id}-action"))
      .variant(CardsText::Action.role())
      .selectable(true)
      .width(Dimension::Pct(100.0))
      .into()
  });
  let agent = request.agent.as_str();
  let reason = if came.reason == PersonReason::ReviewerCouldNotRun {
    could_not_run::line(ctx, id, (place, agent), (came, shown))
  } else {
    came::line(ctx, id, (place, agent), came)
  };
  action
    .into_iter()
    .chain([reason])
    .chain(reviewing.then(|| note(&format!("{id}-reviewing"), &ctx.t("cards.approval.reviewing")).into()))
    .collect()
}

/// *Approve command?* or *Approve tool action?*, and since when it waits.
pub(super) fn head(ctx: &Ctx, id: &str, request: &PendingToolApproval) -> Element {
  let title = ctx.t(if request.command {
    "cards.approval.commandTitle"
  } else {
    "cards.approval.toolTitle"
  });
  let since = Text::new(&ctx.t_args(
    "cards.approval.waitingSince",
    [("time", display::clock(request.requested_at).as_str())],
  ))
  .id(format!("{id}-since"))
  .variant(ExtraTypography::Note.role())
  .nowrap()
  .into();
  card_head(
    id,
    (Icon::ShieldAlert, IconSize::Nav, PaletteColor::Warning),
    (&title, CardsText::ApprovalTitle.role()),
    since,
  )
  .into()
}

/// Everything between the head and the answers. In the Needs you panel the item already says what the agent
/// asks, so the card starts at its subject (`YDJsC`). While a message is written the editor says what the agent is
/// told (`editing`), so the body does not (`D2fdTQ`).
pub(super) fn body(
  ctx: &mut Ctx,
  id: &str,
  (place, request): (&Place, &PendingToolApproval),
  (shown, reviewing, editing): (Option<&ShownApproval>, bool, bool),
) -> Vec<Element> {
  let mut body: Vec<Element> = Vec::new();
  if !place.in_panel {
    let explain = words::explanation(ctx, shown, request);
    body.push(
      Text::new(&explain)
        .id(format!("{id}-explain"))
        .variant(ExtraTypography::Prose.role())
        .width(Dimension::Pct(100.0))
        .into(),
    );
  }
  body.push(subject(ctx, id, request, shown));
  let reason = match words::own_reach(request) {
    Some(reach) => Some(words::own_reason(ctx, reach)),
    None => shown
      .and_then(|shown| shown.choices.reason.as_ref())
      .map(|reason| words::reason(ctx, reason)),
  };
  body.extend(reason.map(|reason| {
    Text::new(&reason)
      .id(format!("{id}-reason"))
      .variant(ExtraTypography::Lead.role())
      .color(PaletteColor::TextPrimary)
      .width(Dimension::Pct(100.0))
      .into()
  }));
  body.extend(reviewing.then(|| note(&format!("{id}-reviewing"), &ctx.t("cards.approval.reviewing")).into()));
  // Why auto mode left it to the person without a review: the hard rule that held it.
  body.extend(
    shown
      .and_then(ShownApproval::hold_cause)
      .map(|cause| note(&format!("{id}-hold-cause"), &message_text(ctx, &cause)).into()),
  );
  body.extend(value_box(ctx, id, request));
  body.push(facts(ctx, id, (place, request), shown));
  if !editing {
    body.push(deny_tells(ctx, id, request, shown));
  }
  body
}

/// The category chip and the setting the request asks under; an MCP call's server and tool.
fn subject(ctx: &Ctx, id: &str, request: &PendingToolApproval, shown: Option<&ShownApproval>) -> Element {
  let chip = match shown.and_then(|shown| Some((shown.choices.category?, shown.choices.secrets))) {
    Some((category, secrets)) if request.command => ctx.t(words::subject_key(category, secrets)),
    _ if request.command => ctx.t("cards.approval.category.unknown"),
    _ => request.server.as_str().into(),
  };
  let policy = shown.and_then(|shown| words::policy(ctx, shown));
  Row::new()
    .id(format!("{id}-subject"))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(TagChip::new(format!("{id}-category"), chip))
    .with_children(policy.map(|policy| {
      Text::new(&policy)
        .id(format!("{id}-policy"))
        .variant(ExtraTypography::Prose.role())
        .nowrap()
    }))
    .into()
}

/// The command whole, or an MCP call's arguments as recorded, in a mono box with Copy.
fn value_box(ctx: &mut Ctx, id: &str, request: &PendingToolApproval) -> Option<Element> {
  let (label, value) = match (request.command, request.scope.as_deref()) {
    (true, _) => (ctx.t("cards.approval.command"), request.action.as_str()),
    (false, Some(arguments)) => (ctx.t("cards.approval.arguments"), arguments),
    (false, None) => return None,
  };
  Some(CodeBox::new(format!("{id}-value"), label, value).render(ctx))
}

/// The agent · step · run · task, the working directory (or an MCP call's tool and target), and the agent's
/// note on a command.
fn facts(
  ctx: &Ctx,
  id: &str,
  (place, request): (&Place, &PendingToolApproval),
  shown: Option<&ShownApproval>,
) -> Element {
  let step = step(place, shown).unwrap_or_default();
  let run = place.run_number().to_string();
  let task = place.detail.task.key().to_string();
  let agent = ctx.t_args(
    "cards.approval.agentValue",
    [
      ("agent", request.agent.as_str()),
      ("step", &*step),
      ("run", run.as_str()),
      ("task", task.as_str()),
    ],
  );
  let row = |name: &str, key: &str, value: &str, kind| detail_row(&format!("{id}-{name}"), &ctx.t(key), value, kind);
  let mut rows = vec![row("agent", "cards.approval.agent", &agent, DetailValue::Text)];
  if request.command {
    // Where the agent runs it, whole, as recorded; else as the repository names it.
    let folder = request
      .target
      .as_deref()
      .or_else(|| shown.and_then(|shown| shown.cwd.as_deref()));
    rows.extend(folder.map(|folder| {
      row(
        "cwd",
        "cards.approval.cwd",
        &words::platform_path(folder),
        DetailValue::Mono,
      )
    }));
    rows.extend(
      request
        .native
        .map(|native| native_rows(ctx, id, native))
        .into_iter()
        .flatten(),
    );
    rows.extend(
      request
        .scope
        .as_deref()
        .map(|said| row("note", "cards.approval.note", said, DetailValue::Text)),
    );
  } else {
    let tool = ctx.t_args(
      "cards.approval.toolValue",
      [("server", request.server.as_str()), ("tool", request.action.as_str())],
    );
    rows.push(row("tool", "cards.approval.tool", &tool, DetailValue::Mono));
    rows.extend(
      request
        .target
        .as_deref()
        .map(|target| row("target", "cards.approval.target", target, DetailValue::Mono)),
    );
  }
  Column::new()
    .id(format!("{id}-facts"))
    .width(Dimension::Pct(100.0))
    .with_children(rows)
    .into()
}

/// What else a native agent's command asks with (04.H9): the shell its string goes to, or none, and how long it may
/// run before Orchester stops it.
fn native_rows(ctx: &Ctx, id: &str, native: NativeCommandAsk) -> [Row; 2] {
  let shell = match native.shell {
    Some(shell) => ctx.t_args(
      "cards.approval.shellValue",
      [("shell", shell.name()), ("invocation", shell.invocation())],
    ),
    None => ctx.t("cards.approval.noShell"),
  };
  let seconds = native.time_limit_seconds.to_string();
  let limit = ctx.t_args("cards.approval.timeLimitSeconds", [("seconds", seconds.as_str())]);
  [
    detail_row(
      &format!("{id}-shell"),
      &ctx.t("cards.approval.shell"),
      &shell,
      DetailValue::Mono,
    ),
    detail_row(
      &format!("{id}-time-limit"),
      &ctx.t("cards.approval.timeLimit"),
      &limit,
      DetailValue::Text,
    ),
  ]
}

/// The words Deny tells the agent, exactly: Orchester's for Claude Code, in its sentence; Codex's own for
/// Codex, under what says so, in mono as Codex words them.
fn deny_tells(ctx: &Ctx, id: &str, request: &PendingToolApproval, shown: Option<&ShownApproval>) -> Element {
  let agent = request.agent.as_str();
  let id = format!("{id}-deny-tells");
  match shown {
    Some(shown) if shown.choices.codex => Column::new()
      .id(id.clone())
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Tight.role())
      .child(note(
        &format!("{id}-label"),
        &ctx.t_args("cards.approval.denyTellsCodexLabel", [("agent", agent)]),
      ))
      .child(
        Text::new(&shown.choices.told)
          .id(format!("{id}-words"))
          .variant(ExtraTypography::MonoKey.role())
          .selectable(true)
          .width(Dimension::Pct(100.0)),
      )
      .into(),
    _ => {
      let told: std::sync::Arc<str> = match shown {
        Some(shown) => shown.choices.told.clone(),
        None => denial_message(request, Answerer::Person).into(),
      };
      note(
        &id,
        &ctx.t_args("cards.approval.denyTells", [("agent", agent), ("message", &*told)]),
      )
      .into()
    }
  }
}
