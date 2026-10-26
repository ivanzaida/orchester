//! A change's card once nobody needs to decide it, folded into its event line (`DECIDED PROPOSALS` in `1 · Event
//! lines` `VGbU7`): being carried out (the loader and *Being carried out*), applied by the person (*Applied by you*,
//! with the task), done by the chat itself (its chip when auto mode or *merge on its own* made it), dismissed or not
//! done (`ban` in `danger`, the tool's reason on hover and once pressed), or merged without Orchester. What it did is
//! said on hover too; the line's link is the first place it opens.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_workspace::messages::Message;

use super::{
  opens::Opens,
  words::{Phase, View, Words, by_you},
};
use crate::{
  navigation::Navigation,
  screens::orchestrator::line::{Chip, ChipTone, Line, LineLink, Tone},
  ui::{icon::Icon, message::message_text, pressable::Press},
};

/// The line of `view`, named `id`, with what `words` say, without its time when `listed` in its answer's list; `opens`
/// gives its link, and `failed` why reading or deciding it failed, said on hover.
pub(super) fn line(
  ctx: &mut Ctx,
  (id, view, listed): (&str, &View, bool),
  words: &Words,
  (opens, failed): (Vec<Opens>, Option<Message>),
) -> Element {
  let task = view.action.task.as_deref().filter(|task| !task.is_empty());
  let (glyph, tone) = look(view.phase(), words.settled.is_some());
  let by = match (&words.settled, by_you(ctx, view)) {
    (Some(settled), _) => Some(settled.clone()),
    (None, Some(by)) => Some(by),
    (None, None) => None,
  };
  let target = match (task, by) {
    (Some(task), Some(by)) => Some(ctx.t_args(
      "orchestrator.line.targetBy",
      [("target", task), ("by", &*by)],
    )),
    (Some(task), None) => Some(task.into()),
    (None, by) => by,
  };
  let chip = match view.phase() {
    Phase::Applying => Some(Chip::new(ctx.t("orchestrator.line.applying"), ChipTone::Working)),
    _ => words.chip.map(|chip| Chip::new(ctx.t(chip.words()), ChipTone::Neutral)),
  };
  // A proposal the person approved in the chat quotes the message that did (ORC-96).
  let approved = view.action.approved_in.as_ref().map(|approval| {
    ctx.t_args(
      "orchestrator.change.approvedInChat",
      [("words", approval.words.as_str())],
    )
  });
  let detail = match (view.phase(), failed, approved) {
    (_, Some(failed), _) => Some(message_text(ctx, &failed)),
    (Phase::Applying, None, _) => None,
    (_, None, Some(approved)) => Some(approved),
    (_, None, None) => Some(words.effect.clone()),
  };
  let navigation = Navigation::of(ctx);
  let link = opens.into_iter().next().map(|(_, label, route)| LineLink {
    label: ctx.t_args("orchestrator.line.link", [("label", &*label)]),
    press: Press::Open(navigation.to(route)),
  });
  Line::new(id, (glyph, tone), words.title.clone())
    .outcome(view.phase().name())
    .target(target)
    .chip(chip)
    .time((!listed).then(|| view.time()))
    .link(link)
    .detail(detail)
    .render(ctx)
}

/// The icon and tone of a line in `phase`; `settled` for one nobody decided that no longer waits.
const fn look(phase: Phase, settled: bool) -> (Icon, Tone) {
  match phase {
    Phase::Waits if settled => (Icon::CircleCheck, Tone::Success),
    Phase::Waits => (Icon::Hourglass, Tone::Warning),
    Phase::Applying => (Icon::LoaderCircle, Tone::Plain),
    Phase::Applied | Phase::Done => (Icon::CircleCheck, Tone::Success),
    Phase::Dismissed => (Icon::X, Tone::Muted),
    Phase::Refused(_) => (Icon::Ban, Tone::Danger),
  }
}
