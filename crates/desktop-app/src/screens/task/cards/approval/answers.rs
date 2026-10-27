//! A waiting request's answers, as the Needs-you panel answers it in place (`Answers` `LZ4NI` and `Wider`
//! `WNoFY` in `YDJsC`): *Deny with a message* at the left, then *Deny* and the primary *Approve once*; under
//! *Or answer more widely* a tile each for *Approve similar in this run*, *Approve everything in this run* and
//! *Always allow…*, saying what it covers before it is chosen; for Orchester's `start_qa_app` also *Approve for every
//! run of this project* (the person's decision of 2026-10-18), which keeps a rule of the project while the QA script
//! is unchanged. An answer the request does not offer stays in
//! place, faded, with a lock and why. *Always allow…* is not built (D6); *Deny with a message* opens its editor in
//! place of the answers ([`super::message`]), but for a native agent's command, whose denial carries no words yet.
//!
//! A wider answer lets the run's agents do more, so it is a person's (`docs/stable-build.md`): its tile asks in
//! place under the tiles, as a Connections page asks what a save widens (`In-place confirmation` `RWODj`), and
//! only that question's confirm sends it. That confirm refuses an MCP client's press unless the person let
//! automation answer approvals, and the card refuses a wider answer from one even then; the rest of the window,
//! *Keep asking* included, takes a client's input, so a client can take the question back. The tile being confirmed
//! stays marked (`$hover` fill, `$border_focus` stroke) while the question shows.

use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Spacer, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::RunId,
  run::{ApprovalAnswer, PendingToolApproval, ProjectScope},
};
use orchester_ipc::Command;
use orchester_workspace::{
  tasks::approvals::{ShownApproval, SimilarShown},
  workspace::OpenProject,
};

use super::{
  super::{
    decisions::{Decisions, Given, Stage},
    frame::{held, note},
  },
  words,
};
use crate::{
  mcp::input_gate::Guarded,
  screens::connections::widen::{WidenConfirm, WidenLook, WidenProps},
  theme::{DISABLED_OPACITY, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    pressable::{Focus, inert},
    unavailable::reason_line,
  },
};

/// The runner's answer for what the person gave on the card, as its kind: a denial with a message is a denial, whose
/// words its editor sends with it ([`Answering::send_message`]).
pub(super) const fn approval_answer(given: Given) -> ApprovalAnswer {
  match given {
    Given::ApproveSimilar => ApprovalAnswer::Similar,
    Given::ApproveProject => ApprovalAnswer::SimilarProject,
    Given::ApproveEverything => ApprovalAnswer::ApproveRun,
    Given::Deny | Given::DenyWithMessage => ApprovalAnswer::Deny,
    Given::ApproveOnce | Given::Answers | Given::ApprovePlan => ApprovalAnswer::Once,
  }
}

/// What the answers act on: the card's key and element id, the request, its choices once read, the run's
/// number, the open project its runner takes the answer for, the tab's answers, *Approve once*, which
/// takes the focus when the tab opens on the card, the wider answer the card asks to confirm, if any, and the
/// message editor's state: whether it shows, what is written, its field and whether that takes the focus.
pub(super) struct Answering<'a> {
  pub(super) card: (RunId, &'a Arc<str>),
  pub(super) id: &'a str,
  pub(super) request: &'a PendingToolApproval,
  pub(super) shown: Option<&'a ShownApproval>,
  pub(super) run: u32,
  pub(super) open: Option<OpenProject>,
  pub(super) decisions: &'a Decisions,
  pub(super) once: &'a ElementRef,
  pub(super) asking: &'a Signal<Option<Given>>,
  pub(super) editing: &'a Signal<bool>,
  pub(super) message: &'a Signal<String>,
  pub(super) field: &'a ElementRef,
  pub(super) focus_field: &'a Arc<AtomicBool>,
}

impl Answering<'_> {
  /// What the card sent last, while it is on its way.
  pub(super) fn sending(&self) -> Option<Given> {
    self
      .decisions
      .outcome(self.card)
      .filter(|outcome| outcome.stage == Stage::Sending)
      .map(|outcome| outcome.given)
  }

  /// Sends `given`, by whoever's input pressed it ([`Decisions::answer`]); nothing while another answer is on its
  /// way.
  fn press(&self, given: Given, open: OpenProject) -> impl Fn() + Send + Sync + 'static {
    let (decisions, (run, key)) = (self.decisions.clone(), (self.card.0, self.card.1.clone()));
    let (request, approval) = (self.request.run, self.request.id.clone());
    move || {
      let command = |by| Command::DecideToolApproval {
        run: request,
        approval: approval.clone(),
        answer: approval_answer(given),
        by,
      };
      decisions.answer((run, &key), given, (&open, command));
    }
  }

  /// Whether `given` can be pressed now, and how: loading while it is on its way, held while another is.
  pub(super) fn hold(&self, given: Given) -> Result<OpenProject, bool> {
    if self.sending() == Some(given) {
      return Err(true);
    }
    match &self.open {
      Some(open) if !self.decisions.busy() => Ok(open.clone()),
      _ => Err(false),
    }
  }

  /// `given`'s button.
  fn button(&self, ctx: &Ctx, given: Given, (kind, glyph, key): (ButtonKind, Icon, &str)) -> ActionButton {
    let id = match given {
      Given::Deny => "deny",
      _ => "approve",
    };
    let button = ActionButton::new(format!("{}-{id}", self.id), kind, ctx.t(key))
      .icon(glyph)
      .in_form();
    match self.hold(given) {
      Err(true) => button.loading(),
      Err(false) => button.disabled(),
      Ok(open) => button.on_click(self.press(given, open)),
    }
  }
}

pub(super) fn answers(ctx: &mut Ctx, answering: &Answering<'_>) -> Element {
  let id = answering.id;
  let (with_message, reason) = with_message(ctx, answering);
  let deny = answering.button(
    ctx,
    Given::Deny,
    (ButtonKind::Secondary, Icon::Ban, "cards.approval.deny"),
  );
  let once = answering
    .button(
      ctx,
      Given::ApproveOnce,
      (ButtonKind::Primary, Icon::Check, "cards.approval.approveOnce"),
    )
    .element_ref(answering.once.clone());
  let row = Row::new()
    .id(format!("{id}-answers"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(with_message)
    .child(Spacer::new().flex(1.0))
    .child(deny)
    .child(once);
  let wider = wider(ctx, answering);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(row)
    .with_children((answering.decisions.busy() && answering.sending().is_none()).then(|| held(ctx, id)))
    .with_children(reason)
    .child(wider)
    .into()
}

/// *Deny with a message*, which opens its editor; for a native agent's command, whose denial carries no words yet,
/// locked, with why under the answers.
fn with_message(ctx: &Ctx, answering: &Answering<'_>) -> (ActionButton, Option<Element>) {
  let id = answering.id;
  let button = ActionButton::new(
    format!("{id}-deny-message"),
    ButtonKind::Ghost,
    ctx.t("cards.approval.denyWithMessage"),
  )
  .icon(Icon::MessageSquareText)
  .in_form();
  if answering.request.native.is_some() {
    let why = ctx.t_args(
      "cards.approval.denyWithMessageNative",
      [("agent", answering.request.agent.as_str())],
    );
    let reason = reason_line(&format!("{id}-deny-message-reason"), &why).into();
    return (button.unavailable(why), Some(reason));
  }
  if answering.hold(Given::DenyWithMessage).is_err() {
    return (button.disabled(), None);
  }
  let (editing, focus) = (answering.editing.clone(), answering.focus_field.clone());
  let button = button.on_click(move || {
    focus.store(true, Ordering::Relaxed);
    editing.set(true);
  });
  (button, None)
}

/// *Or answer more widely*: Approve similar, Approve everything in this run, Always allow; then the question
/// that confirms the one chosen.
fn wider(ctx: &mut Ctx, answering: &Answering<'_>) -> Element {
  let id = answering.id;
  let run = answering.run.to_string();
  let similar = similar_covers(ctx, answering, &run);
  let everything = match answering.shown {
    Some(shown) if shown.auto_approving && shown.choices.auto_asks => {
      Err(ctx.t_args("cards.approval.everythingOn", [("run", run.as_str())]))
    }
    Some(shown) if shown.auto_approving => {
      Err(ctx.t_args("cards.approval.everythingOnEarlier", [("run", run.as_str())]))
    }
    Some(_) => Ok(ctx.t_args("cards.approval.everythingCovers", [("run", run.as_str())])),
    None => Err(ctx.t("cards.approval.unread")),
  };
  let always = ctx.t(if answering.request.command {
    "cards.approval.alwaysCommand"
  } else {
    "cards.approval.alwaysTool"
  });
  let project = project_covers(ctx, answering.request);
  let asked = match answering.asking.get() {
    Some(Given::ApproveSimilar) => similar.clone().ok().map(|covers| (Given::ApproveSimilar, covers)),
    Some(Given::ApproveProject) => project.clone().map(|covers| (Given::ApproveProject, covers)),
    Some(Given::ApproveEverything) => everything.clone().ok().map(|covers| (Given::ApproveEverything, covers)),
    _ => None,
  };
  let question = asked.and_then(|(given, covers)| confirm_wider(ctx, answering, given, covers));
  Column::new()
    .id(format!("{id}-wider"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(note(&format!("{id}-wider-label"), &ctx.t("cards.approval.wider")))
    .child(tile(
      ctx,
      answering,
      (Given::ApproveSimilar, Icon::ShieldCheck, "cards.approval.similar"),
      similar,
    ))
    .with_children(project.map(|covers| {
      tile(
        ctx,
        answering,
        (Given::ApproveProject, Icon::ShieldCheck, "cards.approval.project"),
        Ok(covers),
      )
    }))
    .child(tile(
      ctx,
      answering,
      (Given::ApproveEverything, Icon::Zap, "cards.approval.everything"),
      everything,
    ))
    .child(locked(
      &format!("{id}-always"),
      &ctx.t("cards.approval.always"),
      &always,
    ))
    .with_children(question)
    .into()
}

/// What *Approve similar in this run* covers, or why it is not offered.
fn similar_covers(ctx: &Ctx, answering: &Answering<'_>, run: &str) -> Result<Arc<str>, Arc<str>> {
  match answering.shown.map(|shown| &shown.choices.similar) {
    Some(Ok(SimilarShown::Command { words, category })) => Ok(ctx.t_args(
      "cards.approval.similarCommand",
      [
        ("words", &**words),
        ("category", &*ctx.t(words::subject_key(*category, false))),
        ("run", run),
      ],
    )),
    Some(Ok(SimilarShown::Tool { server, tool })) => Ok(ctx.t_args(
      "cards.approval.similarTool",
      [("server", &**server), ("tool", &**tool), ("run", run)],
    )),
    Some(Ok(SimilarShown::Folder { folder })) => Ok(ctx.t_args(
      "cards.approval.similarFolder",
      [("folder", words::platform_path(folder).as_str()), ("run", run)],
    )),
    Some(Ok(SimilarShown::Check { name })) => {
      Ok(ctx.t_args("cards.approval.similarCheck", [("check", &**name), ("run", run)]))
    }
    Some(Err(unavailable)) => Err(ctx.t(words::similar_unavailable(*unavailable))),
    None => Err(ctx.t("cards.approval.unread")),
  }
}

/// A tile that gives `given` and says what it covers, or, faded with a lock, why it is not offered.
fn tile(
  ctx: &Ctx,
  answering: &Answering<'_>,
  (given, glyph, title): (Given, Icon, &str),
  covers: Result<Arc<str>, Arc<str>>,
) -> Element {
  let id = format!("{}-{}", answering.id, wider_name(given));
  let covers = match covers {
    Ok(covers) => covers,
    Err(reason) => return locked(&id, &ctx.t(title), &reason),
  };
  if let Err(under_way) = answering.hold(given) {
    let glyph = if under_way { Icon::LoaderCircle } else { glyph };
    return inert(tile_frame(&id, glyph, (&ctx.t(title), &covers)))
      .opacity(DISABLED_OPACITY)
      .describe("state", if under_way { "loading" } else { "disabled" })
      .into();
  }
  let asking = answering.asking.clone();
  let asked = asking.get() == Some(given);
  let frame = tile_frame(&id, glyph, (&ctx.t(title), &covers));
  // The tile being confirmed stays marked once the pointer leaves it: the hover fill in a `border_focus` stroke.
  let frame = if asked {
    frame
      .background(ExtraColor::Hover.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderFocus))
  } else {
    frame
  };
  Focus::Keyboard
    .apply(frame)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .cursor(CursorIcon::Pointer)
    .describe("state", if asked { "asking" } else { "enabled" })
    .on_click(move |_| asking.set(Some(given)))
    .into()
}

/// The question that confirms the wider answer the card asks about, under its tiles, while it can be sent: what
/// it covers, confirm, and *Keep asking*, which takes the question back.
fn confirm_wider(ctx: &mut Ctx, answering: &Answering<'_>, given: Given, covers: Arc<str>) -> Option<Element> {
  let open = answering.hold(given).ok()?;
  let (title, confirm) = match given {
    Given::ApproveSimilar => ("cards.approval.confirm.similarTitle", "cards.approval.confirm.similar"),
    Given::ApproveProject => ("cards.approval.confirm.projectTitle", "cards.approval.confirm.project"),
    Given::ApproveEverything => (
      "cards.approval.confirm.everythingTitle",
      "cards.approval.confirm.everything",
    ),
    Given::ApproveOnce | Given::Deny | Given::DenyWithMessage | Given::Answers | Given::ApprovePlan => return None,
  };
  let name = wider_name(given);
  let run = answering.run.to_string();
  let press = answering.press(given, open);
  let (sent, kept) = (answering.asking.clone(), answering.asking.clone());
  Some(ctx.mount_keyed::<WidenConfirm>(
    &format!("{}-widen-{name}", answering.id),
    WidenProps {
      id: format!("{}-widen", answering.id).into(),
      look: WidenLook::Card,
      affirming: given == Given::ApproveProject,
      automation: Guarded::ApprovalWidening,
      title: ctx.t_args(
        title,
        [("run", run.as_str()), ("tool", answering.request.action.as_str())],
      ),
      reasons: vec![covers].into(),
      confirm: ctx.t(confirm),
      keep: ctx.t("cards.approval.confirm.keep"),
      on_confirm: Arc::new(move || {
        sent.set(None);
        press();
      }),
      on_keep: Arc::new(move || kept.set(None)),
    },
  ))
}

/// The element name of a wider answer's tile and question.
const fn wider_name(given: Given) -> &'static str {
  match given {
    Given::ApproveProject => "project",
    Given::ApproveEverything => "everything",
    _ => "similar",
  }
}

/// What *Approve for every run of this project* covers, for a request it is offered ([`ProjectScope`]).
fn project_covers(ctx: &Ctx, request: &PendingToolApproval) -> Option<Arc<str>> {
  let scope = ProjectScope::of_request(request)?;
  Some(ctx.t_args(
    "cards.approval.projectCovers",
    [
      ("server", scope.server()),
      ("tool", scope.tool()),
      ("script", scope.short_script()),
    ],
  ))
}

/// A tile for an answer that is not offered: faded, a lock for its glyph, and why.
fn locked(id: &str, title: &str, reason: &str) -> Element {
  inert(tile_frame(id, Icon::Lock, (title, reason)))
    .opacity(DISABLED_OPACITY)
    .describe("state", "unavailable")
    .describe("reason", reason.to_owned())
    .into()
}

/// A tile (`tpivE`): a 14 glyph, its title in 12/500 and what it covers in 11 `text_muted` under it, 8 by 10
/// inside a `border` stroke, radius 6.
fn tile_frame(id: &str, glyph: Icon, (title, covers): (&str, &str)) -> Button {
  Button::empty()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .describe("a11y_name", title.to_owned())
    .child(icon(glyph, IconSize::Md, PaletteColor::TextSecondary))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(title)
            .id(format!("{id}-title"))
            .variant(TypographyStyle::Label),
        )
        .child(
          Text::new(covers)
            .id(format!("{id}-covers"))
            .variant(TypographyStyle::Caption)
            .width(Dimension::Pct(100.0)),
        ),
    )
}

/// What automation may do with this request through Orchester's MCP server.
pub(super) fn automation(ctx: &Ctx, id: &str, shown: Option<&ShownApproval>) -> Element {
  let key = if shown.is_some_and(|shown| shown.choices.auto_asks) {
    "cards.approval.automationDenyOnly"
  } else {
    "cards.approval.automationOnceOrDeny"
  };
  Text::new(&ctx.t(key))
    .id(format!("{id}-automation"))
    .variant(ExtraTypography::Note.role())
    .width(Dimension::Pct(100.0))
    .into()
}
