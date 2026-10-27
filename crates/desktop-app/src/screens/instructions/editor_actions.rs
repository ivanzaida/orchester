//! What an instruction's page shows besides its fields: a write that failed with its retry, what publishing kept for
//! runs already under way, the reload of a rule that changed elsewhere, and Discard draft and Remove asked in place.
use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx, theme::SpacingSize},
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  resource::{Instruction, ResourceKind},
  run::PromptPart,
};
use orchester_workspace::{
  agents::data::VersionState,
  instructions::{data::InstructionEntry, publication::RetainedRun},
  tasks::outside::Outside,
};

use super::{
  editor::{Editor, Props, may_publish},
  state::View,
  words,
  work::{Change, Failure, Write},
};
use crate::{
  screens::hub::{discard, removal},
  theme::HubText,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
  },
};

/// A question that opens under the head about the rule itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask {
  Nothing,
  Discard,
  Remove,
}
impl DevtoolsInspectable for Ask {}

pub(super) fn write_error(editor: &Editor, ctx: &Ctx, pane: Column) -> Column {
  let state = editor.work.action.state().get();
  let Some(error) = state.error.clone().filter(|_| state.is_rejected()) else {
    return pane;
  };
  let (retry, gate) = (editor.work.clone(), editor.gate.clone());
  let candidate = editor.work.retry(&editor.fields);
  let valid = candidate.is_some();
  let retry = ActionButton::new("instruction-retry", ButtonKind::Secondary, ctx.t("instructions.retry")).on_click(move || {
    let Some(candidate) = &candidate else {
      return;
    };
    if !matches!(candidate.change(), Some(Change::Publish(..))) || may_publish(gate.as_ref()) {
      retry.run(candidate.clone());
    }
  });
  let (title, explanation) = if editor.work.publication_failed() {
    words::publication_failure(
      ctx,
      editor.saved.get().0.as_ref().and_then(|entry| entry.state.published()),
      error.message(),
    )
  } else {
    (ctx.t("instructions.writeFailed"), message_text(ctx, error.message()))
  };
  // What was typed stays: the `danger` icon only, no red fill (`SsC16`).
  let notice = Notice::new("instruction-write-error", NoticeTone::Info, title)
    .icon(Icon::CircleAlert)
    .mark(lurq::app::theme::PaletteColor::Danger)
    .body(explanation)
    .action(if valid { retry } else { retry.disabled() });
  pane.child(notice.stacked())
}

pub(super) fn published_notice(editor: &Editor, ctx: &mut Ctx, retained: &[RetainedRun], pane: Column) -> Column {
  let Some(version) = editor.published.get() else {
    return pane;
  };
  let dismiss = editor.published.clone();
  let dismiss = crate::ui::icon_button::IconButton::new(
    "instruction-published-dismiss",
    Icon::X,
    ctx.t("instructions.closePanel"),
  )
  .small()
  .on_click(move || dismiss.set(None))
  .render(ctx);
  pane.child(crate::ui::notice::result_notice(
    "instruction-published-notice",
    &words::published_notice(ctx, version, retained),
    dismiss,
  ))
}

pub(super) fn reload(
  editor: &Editor,
  ctx: &Ctx,
  props: &Props,
  (saved, busy): (Option<&InstructionEntry>, bool),
  pane: Column,
) -> Column {
  let Some(entry) = saved else {
    return pane;
  };
  let state = editor.work.action.state().get();
  let conflict = matches!(state.error, Some(Failure::Conflict(_)));
  let retry_keeps_draft = editor.work.publication_failed()
    && !conflict
    && matches!(
      editor.work.retry(&editor.fields).as_ref().and_then(Write::change),
      Some(Change::Publish(..))
    );
  if props.entry.as_ref() == Some(entry)
    && (!state.is_rejected() || retry_keeps_draft)
    && !matches!(entry.state, VersionState::StaleDraft { .. })
  {
    return pane;
  }
  let (work, id, fields) = (editor.work.clone(), entry.id, editor.fields.clone());
  let reload = ActionButton::new("instruction-reload", ButtonKind::Secondary, ctx.t("instructions.reload"))
    .icon(Icon::RotateCcw)
    .on_click(move || work.run(Write::Reload(id, fields.snapshot())));
  pane.child(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .with_children(
        (props.entry.as_ref() != Some(entry))
          .then(|| Text::new(&ctx.t("instructions.outsideChange")).variant(HubText::Hint.role())),
      )
      .child(if busy { reload.disabled() } else { reload }),
  )
}

/// Discard draft, asked in place of the draft bar.
pub(super) fn question(editor: &Editor, ctx: &mut Ctx, props: &Props, saved: Option<&InstructionEntry>) -> Option<Element> {
  let entry = saved.filter(|_| editor.ask.get() == Ask::Discard)?;
  let published = entry.state.published().map_or(0, |v| v.get());
  let (root, id, expected) = (props.project.open.repo_root.clone(), entry.id, entry.body.clone());
  let (keep, done, work, fields) = (editor.ask.clone(), editor.ask.clone(), editor.work.clone(), editor.fields.clone());
  Some(discard::question(
    ctx,
    discard::DiscardProps {
      project: props.project.clone(),
      versions: (published + 1, published),
      discard: std::sync::Arc::new(move || {
        orchester_workspace::removal::discard_in::<Instruction>(&root, id, &expected)
      }),
      change: Outside::Instructions,
      on_keep: std::sync::Arc::new(move || keep.set(Ask::Nothing)),
      on_discarded: std::sync::Arc::new(move || {
        done.set(Ask::Nothing);
        // The fields take the published version back: the same reload a changed rule offers.
        work.run(Write::Reload(id, fields.snapshot()));
      }),
    },
  ))
}

/// Remove, asked under the head; a removed rule leaves a *Removed* line in its group of the list.
pub(super) fn remove_question(editor: &Editor, ctx: &mut Ctx, props: &Props, saved: Option<&InstructionEntry>) -> Option<Element> {
  let entry = saved.filter(|_| editor.ask.get() == Ask::Remove)?;
  let (keep, selection) = (editor.ask.clone(), props.selection.clone());
  let (part, name) = (PromptPart::of(entry.body.scope()), entry.name.clone());
  Some(removal::question(
    ctx,
    removal::RemoveProps {
      project: props.project.clone(),
      resource: entry.id,
      kind: ResourceKind::Instruction,
      name: entry.name.clone(),
      on_keep: std::sync::Arc::new(move || keep.set(Ask::Nothing)),
      on_removed: std::sync::Arc::new(move || {
        selection.removed.update(|removed| removed.0.push((part, name.clone())));
        selection.dirty.set(false);
        selection.saved(View::default());
      }),
    },
  ))
}
