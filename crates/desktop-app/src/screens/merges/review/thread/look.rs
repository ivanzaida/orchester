//! How a thread's state shows in its head (the tags of `RBR9O`): *Sent to Implement* and *Implement is answering* on
//! `success_muted` with a turning loader, *Answered · no code changed*, *Stopped by you*, *Not asked* and *Not sent* on
//! `warning_muted`, *Sending…*, *Sent · Implement is changing the code* on `info_muted`, *Addressed in 9a1b2c3*, and a
//! folded thread's *Resolved* or *Outdated*. A thread's kind gives its glyph, word and stroke: a comment `info`, a
//! question `success`.

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::review::{Said, ThreadKind, ThreadState};
use orchester_workspace::{display::short_revision, tasks::review_threads::read::ThreadView};

use super::super::tag::TagLook;
use crate::ui::icon::Icon;

/// A thread's kind: its glyph, its word's key and its color.
pub(super) const fn kind(kind: ThreadKind) -> (Icon, &'static str, PaletteColor) {
  match kind {
    ThreadKind::Comment => (Icon::MessageSquare, "review_page.thread.comment", PaletteColor::Info),
    ThreadKind::Question => (Icon::CircleHelp, "review_page.thread.question", PaletteColor::Success),
  }
}

/// Whether Implement wrote any words after the person's latest question.
pub(super) fn words_arrived(view: &ThreadView) -> bool {
  let messages = view.thread.messages();
  let latest = messages.iter().rposition(|message| message.said().is_person());
  latest.is_some_and(|at| {
    messages[at + 1..]
      .iter()
      .any(|message| matches!(message.said(), Said::Agent(_)) && !message.body().trim().is_empty())
  })
}

/// The tag of `view`'s state: its words and look.
pub(super) fn tag(ctx: &Ctx, view: &ThreadView) -> (Arc<str>, TagLook) {
  let success = TagLook::new(PaletteColor::SuccessMuted, PaletteColor::Success);
  let warning = TagLook::new(PaletteColor::WarningMuted, PaletteColor::Warning);
  let quiet = TagLook::new(PaletteColor::SurfacePanel, PaletteColor::TextMuted);
  match &view.state {
    ThreadState::Answering if words_arrived(view) => {
      (ctx.t("review_page.tag.answering"), success.glyph(Icon::LoaderCircle))
    }
    ThreadState::Asked | ThreadState::Answering => (
      ctx.t("review_page.tag.sentToImplement"),
      success.glyph(Icon::LoaderCircle),
    ),
    ThreadState::Answered => (ctx.t("review_page.tag.answered"), success.glyph(Icon::Check)),
    ThreadState::AnswerStopped => (ctx.t("review_page.tag.stopped"), quiet.glyph(Icon::CircleStop)),
    ThreadState::NotAsked(_) => (ctx.t("review_page.tag.notAsked"), warning),
    ThreadState::NotSent | ThreadState::CouldNotSend(_) => (ctx.t("review_page.tag.notSent"), warning),
    ThreadState::Sending => (ctx.t("review_page.tag.sending"), quiet.glyph(Icon::LoaderCircle)),
    ThreadState::Sent => (
      ctx.t("review_page.tag.sent"),
      TagLook::new(PaletteColor::InfoMuted, PaletteColor::Info).glyph(Icon::LoaderCircle),
    ),
    ThreadState::Addressed { commit: Some(commit) } => (
      ctx.t_args(
        "review_page.tag.addressedIn",
        [("commit", short_revision(commit.as_str()))],
      ),
      success.glyph(Icon::Check),
    ),
    ThreadState::Addressed { commit: None } => (ctx.t("review_page.tag.addressed"), success.glyph(Icon::Check)),
    ThreadState::Resolved => (ctx.t("review_page.tag.resolved"), quiet.glyph(Icon::Check)),
    ThreadState::Outdated { commit } => (
      ctx.t_args(
        "review_page.tag.outdatedIn",
        [("commit", short_revision(commit.as_str()))],
      ),
      quiet.glyph(Icon::History),
    ),
  }
}
