//! What a waiting tool request asks (`Approval card` `gwOaJ`, with the agent's note of `YDJsC`): *Approve
//! command?* and since when it waits; what the agent wants and what waiting does; its category chip and the
//! setting it asks under; why the classifier asks; the command whole, or an MCP call's arguments, in a mono
//! box with Copy; the agent · step · run · task, the working directory and the agent's note; and the words
//! Deny tells the agent.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::run::{Answerer, PendingToolApproval, denial_message};
use orchester_workspace::{display, tasks::approvals::ShownApproval};

use super::{
  super::{
    Place,
    frame::{head as card_head, note},
  },
  words,
};
use crate::{
  theme::{CardsText, ExtraTypography, IconSize},
  ui::{
    chip::TagChip,
    code_box::CodeBox,
    detail_row::{DetailValue, detail_row},
    icon::Icon,
  },
};

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

/// Everything between the head and the answers.
pub(super) fn body(
  ctx: &mut Ctx,
  id: &str,
  (place, request): (&Place, &PendingToolApproval),
  (shown, reviewing): (Option<&ShownApproval>, bool),
) -> Vec<Element> {
  let explain = words::explanation(ctx, shown, (&request.agent, &request.server, request.command));
  let mut body: Vec<Element> = vec![
    Text::new(&explain)
      .id(format!("{id}-explain"))
      .variant(ExtraTypography::Prose.role())
      .width(Dimension::Pct(100.0))
      .into(),
    subject(ctx, id, request, shown),
  ];
  let reason = shown.and_then(|shown| shown.choices.reason.as_ref());
  body.extend(reason.map(|reason| {
    Text::new(&words::reason(ctx, reason))
      .id(format!("{id}-reason"))
      .variant(ExtraTypography::Lead.role())
      .color(PaletteColor::TextPrimary)
      .width(Dimension::Pct(100.0))
      .into()
  }));
  body.extend(reviewing.then(|| note(&format!("{id}-reviewing"), &ctx.t("cards.approval.reviewing")).into()));
  body.extend(value_box(ctx, id, request));
  body.push(facts(ctx, id, (place, request), shown));
  body.push(note(&format!("{id}-deny-tells"), &deny_tells(ctx, request, shown)).into());
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
  let step = shown
    .and_then(|shown| shown.step.clone())
    .or_else(|| place.step.clone())
    .unwrap_or_default();
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
    let folder = shown
      .and_then(|shown| shown.cwd.as_deref())
      .or(request.target.as_deref());
    rows.extend(folder.map(|folder| row("cwd", "cards.approval.cwd", folder, DetailValue::Mono)));
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

/// The words Deny tells the agent, exactly: Orchester's for Claude Code, Codex's own for Codex.
fn deny_tells(ctx: &Ctx, request: &PendingToolApproval, shown: Option<&ShownApproval>) -> std::sync::Arc<str> {
  let (key, told) = match shown {
    Some(shown) if shown.choices.codex => ("cards.approval.denyTellsCodex", shown.choices.told.clone()),
    Some(shown) => ("cards.approval.denyTells", shown.choices.told.clone()),
    None => (
      "cards.approval.denyTells",
      denial_message(request, Answerer::Person).into(),
    ),
  };
  ctx.t_args(key, [("agent", request.agent.as_str()), ("message", &*told)])
}
