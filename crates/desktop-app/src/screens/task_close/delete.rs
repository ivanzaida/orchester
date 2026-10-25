//! *Delete KON-12?*, in place (no frame yet: it follows Run again's decision card and the Memory page's *Delete this
//! record?* `YdVse`): what goes with the task, read when it opens (its runs with their history, its conversation and
//! artifacts, its memory records, the project's records and approval rules from its runs, the links of the tasks it
//! blocks), that its worktrees and merged branches are removed while merged work stays in git, and the choice to
//! remove its unmerged work too, off each time it opens. A task that cannot be deleted now says why, and Delete is
//! unavailable. Only a person deletes: an MCP client's press is refused, and the card says so.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::TaskId;
use orchester_exec::programs::Programs;
use orchester_workspace::{
  background,
  messages::Message,
  tasks::delete::{DeletionPreview, preview_in},
};

use super::{
  Asked, Closer, Subject,
  confirm::{actions, counted},
};
use crate::{
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, IconSize, TaskText},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
    tinted_note::person_only,
    toggle::{Toggle, ToggleKind},
  },
};

/// The confirmation of deleting `subject`, its parts' ids starting with `id`.
pub fn confirmation(ctx: &mut Ctx, closer: &Closer, subject: &Subject, id: &str) -> Element {
  ctx.mount_keyed::<DeleteCard>(
    &format!("{id}-delete-{}", subject.id),
    DeleteProps {
      closer: closer.clone(),
      subject: subject.clone(),
      id: Arc::from(format!("{id}-delete")),
    },
  )
}

#[derive(Clone)]
struct DeleteProps {
  closer: Closer,
  subject: Subject,
  id: Arc<str>,
}

impl PartialEq for DeleteProps {
  fn eq(&self, other: &Self) -> bool {
    self.subject == other.subject && self.id == other.id && self.closer.root == other.closer.root
  }
}

impl DevtoolsInspectable for DeleteProps {}

struct DeleteCard;

impl Component for DeleteCard {
  type Props = DeleteProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DeleteProps>().clone();
    let (closer, id) = (&props.closer, &*props.id);
    let programs = Services::of(ctx).programs;
    let preview = read(ctx, (closer.root.clone(), programs), props.subject.id);
    let key = &*props.subject.key;
    let head = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::Trash2, IconSize::Md, PaletteColor::Danger))
      .child(
        Text::new(&ctx.t_args("task_close.delete.title", [("task", key)]))
          .id(format!("{id}-title"))
          .variant(TaskText::SectionTitle.role()),
      );
    let body: Vec<Element> = match &preview {
      None => vec![
        Text::new(&ctx.t("task_close.delete.reading"))
          .variant(TypographyStyle::Caption)
          .into(),
      ],
      Some(Err(why)) => vec![field_error(&format!("{id}-unavailable"), &message_text(ctx, why)).into()],
      Some(Ok(preview)) => what_goes(ctx, closer, preview, (key, id)),
    };
    let state = closer.action.state().get();
    let pending = state.is_pending();
    let failed = state
      .error
      .filter(|failed| !pending && failed.task == props.subject.id)
      .map(|failed| field_error(&format!("{id}-error"), &message_text(ctx, &failed.message)));
    let refused = closer
      .person_only
      .get()
      .then(|| person_only(format!("{id}-person-only"), &ctx.t("task_close.delete.personOnly")));
    let deletable = matches!(preview, Some(Ok(_)));
    card(id, CardSurface::Decision)
      .spacing(SpacingSize::Md)
      .child(head)
      .with_children(body)
      .with_children(refused)
      .with_children(failed)
      .child(buttons(ctx, closer, &props.subject, (id, deletable, pending)))
  }
}

/// What deleting the task takes with it, once read; `None` while it is read, why it cannot be deleted when it cannot.
fn read(
  ctx: &mut Ctx,
  (root, programs): (PathBuf, Programs),
  task: TaskId,
) -> Option<Result<DeletionPreview, Message>> {
  let state = ctx
    .future((root, task), move |(root, task)| {
      let programs = programs.clone();
      async move { background::run(move || preview_in(&root, &programs, task)).await? }
    })
    .state()
    .get();
  let pending = state.is_pending();
  match (state.data, state.error) {
    (_, Some(error)) if !pending => Some(Err(error)),
    (Some(preview), _) => Some(Ok(preview)),
    _ => None,
  }
}

/// What goes with the task in whole sentences, what happens in git, and the choice to remove unmerged work too.
fn what_goes(ctx: &Ctx, closer: &Closer, preview: &DeletionPreview, (key, id): (&str, &str)) -> Vec<Element> {
  let facts = preview.facts;
  let runs = match facts.runs {
    0 => ctx.t_args("task_close.delete.runs.none", [("task", key)]),
    1 => ctx.t_args("task_close.delete.runs.one", [("task", key)]),
    count => ctx.t_args(
      "task_close.delete.runs.other",
      [("task", key.to_owned()), ("count", count.to_string())],
    ),
  };
  let mut sentences = vec![runs];
  let more = [
    (
      facts.memories,
      ("task_close.delete.memories.one", "task_close.delete.memories.other"),
    ),
    (
      facts.project_memories,
      (
        "task_close.delete.projectMemories.one",
        "task_close.delete.projectMemories.other",
      ),
    ),
    (
      facts.project_rules,
      (
        "task_close.delete.projectRules.one",
        "task_close.delete.projectRules.other",
      ),
    ),
    (
      facts.blocking,
      ("task_close.delete.blocking.one", "task_close.delete.blocking.other"),
    ),
  ];
  sentences.extend(
    more
      .into_iter()
      .filter(|(count, _)| *count > 0)
      .map(|(count, keys)| counted(ctx, keys, count)),
  );
  sentences.push(ctx.t("task_close.delete.final"));
  let words: Vec<&str> = sentences.iter().map(|sentence| &**sentence).collect();
  let mut shown: Vec<Element> = vec![
    Text::new(&words.join(" "))
      .id(format!("{id}-text"))
      .variant(ExtraTypography::Prose.role())
      .width(Dimension::Pct(100.0))
      .into(),
    Text::new(&ctx.t("task_close.delete.git"))
      .id(format!("{id}-git"))
      .variant(ExtraTypography::Note.role())
      .width(Dimension::Pct(100.0))
      .into(),
  ];
  shown.extend(unmerged(ctx, closer, preview, id));
  shown
}

/// *Also remove work that is not merged*, while the runs left some, with what it is.
fn unmerged(ctx: &Ctx, closer: &Closer, preview: &DeletionPreview, id: &str) -> Option<Element> {
  if !preview.has_unmerged_work() {
    return None;
  }
  let mut notes = Vec::new();
  if preview.changed_worktrees > 0 {
    let keys = (
      "task_close.delete.unmerged.worktrees.one",
      "task_close.delete.unmerged.worktrees.other",
    );
    notes.push(counted(ctx, keys, preview.changed_worktrees));
  }
  if preview.unmerged_branches > 0 {
    let keys = (
      "task_close.delete.unmerged.branches.one",
      "task_close.delete.unmerged.branches.other",
    );
    notes.push(counted(ctx, keys, preview.unmerged_branches));
  }
  notes.push(ctx.t("task_close.delete.unmerged.kept"));
  let words: Vec<&str> = notes.iter().map(|note| &**note).collect();
  let choice = closer.unmerged.clone();
  let toggle = Toggle::new(
    format!("{id}-unmerged"),
    ToggleKind::Checkbox,
    closer.unmerged.get(),
    ctx.t("task_close.delete.unmerged.title"),
  )
  .on_change(move |on| choice.set(on));
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(toggle)
      .child(
        Text::new(&words.join(" "))
          .id(format!("{id}-unmerged-note"))
          .variant(ExtraTypography::Note.role())
          .width(Dimension::Pct(100.0)),
      )
      .into(),
  )
}

/// *Cancel* and *Delete task*: unavailable until what goes with the task was read and it can be deleted, under way
/// while the deletion runs.
fn buttons(ctx: &Ctx, closer: &Closer, subject: &Subject, (id, deletable, pending): (&str, bool, bool)) -> Row {
  let cancel = ActionButton::new(
    format!("{id}-cancel"),
    ButtonKind::Secondary,
    ctx.t("task_close.delete.cancel"),
  )
  .in_form();
  let cancel = if pending {
    cancel.disabled()
  } else {
    let asked = closer.asked.clone();
    cancel.on_click(move || asked.set(Asked::Nothing))
  };
  let delete = ActionButton::new(
    format!("{id}-confirm"),
    ButtonKind::Danger,
    ctx.t("task_close.delete.confirm"),
  )
  .icon(Icon::Trash2)
  .in_form();
  let delete = if pending {
    delete.loading()
  } else if !deletable {
    delete.disabled()
  } else {
    let (closer, subject) = (closer.clone(), subject.clone());
    delete.on_click(move || closer.delete(subject.clone()))
  };
  actions(format!("{id}-actions"), [cancel.into(), delete.into()])
}
