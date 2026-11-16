//! What Edit task says of a team task's save its server did not take as a whole (ORC-734). Someone else's change first
//! is drawn as `Team authority · redesign` 05 `LmxUY` draws it (`Conflict` `Y0ivDH`): a warning callout with the
//! compare icon (`BrTjh`) naming who changed the task and what it is now, and 10 under it, in line with its words
//! (`abHoW`), *Set … again* with the reapply icon and *Keep …*. A change of the priority alone names both values, as
//! the frame does; other changes name what each field the person changed is now. Reapply puts the person's changes on
//! the task as it is now and saves them on its revision; Keep takes the task as it is now, with the person's changes of
//! the other fields. Any other refusal, a member's `not_admin` too, is said in a danger notice.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_api::v2::AuthorityErrorCode;
use orchester_core::task::{
  content::{Choice, ContentField, ContentMerge},
  organization::Priority,
};
use orchester_workspace::{
  actions::PipelineRefusal,
  display,
  tasks::data::{PipelineChoice, SaveTarget},
};

use super::{
  actions::SaveArgs,
  base::Base,
  conflict::{field_key, value_text},
  form::{FormProps, TaskForm},
  state::View,
  team::{Changed, Stop, TeamSave, TeamStop},
  values::Values,
};
use crate::{
  team::act::Acted,
  theme::{ExtraSpacing, ExtraTypography, TaskFormSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    icon::Icon,
    notice::{Notice, NoticeTone},
    status::priority_look,
  },
};

/// A field the person changed, as the callout names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
  Content(ContentField),
  Priority,
}

impl TaskForm {
  /// What the form says over its fields of a team task's save the server did not take, while it edits.
  pub(super) fn team_stop_part(&self, ctx: &Ctx, props: &FormProps, view: &View) -> Option<Element> {
    if view.locked() {
      return None;
    }
    let stop = view.team_stop.as_ref()?.0.clone();
    let key = view.subject.task()?.key().to_string();
    if !stop.changed() {
      return Some(refused(ctx, &stop, &key));
    }
    let Some(current) = &stop.current else {
      let title = ctx.t_args("task_form.team.changed.titleUnread", [("key", key.as_str())]);
      let body = ctx.t_args("task_form.team.changed.unread", [("key", key.as_str())]);
      return Some(callout(title, body, Vec::new()).into());
    };
    Some(self.changed_part(ctx, props, view, (&stop, current, &key)))
  }

  /// Someone else's change first, what the task is now and the two ways on.
  fn changed_part(
    &self,
    ctx: &Ctx,
    props: &FormProps,
    view: &View,
    (stop, current, key): (&TeamStop, &Changed, &str),
  ) -> Element {
    let separator = props.separator.as_ref();
    let base = self.base.get_untracked().map(|base| base.values).unwrap_or_default();
    let mine = changed_fields(&base, &view.values, separator);
    let (name, time) = (current.by.as_str(), display::clock(current.at));
    let title = ctx.t_args("task_form.team.changed.title", [("name", name), ("key", key)]);
    let priority_only = mine == [Field::Priority] && stop.saved.is_none();
    let (body, again, keep, now) = if priority_only {
      let (was, set) = (
        priority_word(ctx, current.task.organization().priority()),
        priority_word(ctx, view.values.priority),
      );
      let body = ctx.t_args(
        "task_form.team.changed.priority",
        [
          ("current", was.as_str()),
          ("name", name),
          ("time", time.as_str()),
          ("mine", set.as_str()),
        ],
      );
      let again = ctx.t_args("task_form.team.changed.setAgain", [("value", set.as_str())]);
      (
        body,
        again,
        ctx.t_args("task_form.team.changed.keepValue", [("value", was.as_str())]),
        Vec::new(),
      )
    } else {
      let body_key = if stop.saved.is_some() {
        "task_form.team.changed.partly"
      } else {
        "task_form.team.changed.fields"
      };
      let body = ctx.t_args(body_key, [("name", name), ("key", key), ("time", time.as_str())]);
      let pipelines = props.pipelines.as_deref().map(Vec::as_slice);
      let now = mine
        .iter()
        .map(|field| now_line(ctx, *field, current, pipelines))
        .collect();
      let keep = ctx.t_args("task_form.team.changed.keepTheirs", [("name", name)]);
      (body, ctx.t("task_form.team.changed.reapply"), keep, now)
    };
    let reapply = self.reapply(props, view, current).map(|reapply| {
      ActionButton::new("task-form-team-conflict-reapply", ButtonKind::Secondary, again)
        .icon(Icon::RotateCcw)
        .on_click(reapply)
    });
    let keep = ActionButton::new("task-form-team-conflict-keep", ButtonKind::Ghost, keep)
      .on_click(self.keep_theirs(separator, view, current));
    let actions = Row::new()
      .id("task-form-team-conflict-actions")
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().left(TaskFormSpacing::ConflictActions.role()))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .with_children(reapply)
      .child(keep);
    Column::new()
      .id("task-form-team-conflict")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .child(callout(title, body, now))
      .child(actions)
      .into()
  }

  /// Reapply: the person's changes put on the task as it is now, theirs where both changed a field, saved on its
  /// revision; `None` while the fields hold what a save refuses.
  fn reapply(&self, props: &FormProps, view: &View, current: &Changed) -> Option<impl Fn() + Send + Sync + 'static> {
    let separator = props.separator.clone();
    let base = self.base.get_untracked()?;
    let made_on = base.values.task_form(&separator).details().ok()?;
    let task = current.task.clone();
    let details = ContentMerge::new(made_on, view.ready.clone()?, task.details().clone()).merged(|_| Choice::Mine);
    let edited = view.values.organization(&base.values, task.organization());
    let module = if view.module == base.module() {
      task.module()
    } else {
      view.module
    };
    let filled = Values {
      no_automatic_merge: edited.no_automatic_merge(),
      priority: edited.priority(),
      delivers: edited.delivers(),
      points: view.values.points,
      ..Values::of_details(&details, &separator)
    };
    let args = SaveArgs {
      repo_root: props.project.open.repo_root.clone(),
      target: SaveTarget::Task(task.id(), task.revision()),
      base: Some(task.details().clone()),
      details,
      organization: (task.organization().clone(), edited),
      points: view.values.points,
      module,
      base_branch: view.base_branch.clone(),
      sources: Vec::new(),
      team: props.team.clone().map(|source| TeamSave {
        source,
        task: task.clone(),
      }),
    };
    let (fields, base, stop) = (self.fields.clone(), self.base.clone(), self.team_stop.clone());
    let (save, dismissed) = (self.save.clone(), self.dismissed.clone());
    Some(move || {
      if save.state().get_untracked().is_pending() {
        return;
      }
      fields.fill(&filled);
      base.set(Some(Base::of(&task, &separator)));
      stop.set(None);
      dismissed.set(false);
      save.run(args.clone());
    })
  }

  /// Keep: the task as it is now, with the person's changes of the fields its change left as they were.
  fn keep_theirs(&self, separator: &str, view: &View, current: &Changed) -> impl Fn() + Send + Sync + 'static {
    let task = current.task.clone();
    let base = self.base.get_untracked();
    let made_on = base
      .as_ref()
      .and_then(|base| base.values.task_form(separator).details().ok());
    let details = match (made_on, view.ready.clone()) {
      (Some(made_on), Some(mine)) => {
        ContentMerge::new(made_on, mine, task.details().clone()).merged(|_| Choice::Theirs)
      }
      _ => task.details().clone(),
    };
    let now = task.organization();
    let base_priority = base.map_or(now.priority(), |base| base.values.priority);
    let filled = Values {
      no_automatic_merge: now.no_automatic_merge(),
      priority: if base_priority == now.priority() {
        view.values.priority
      } else {
        now.priority()
      },
      delivers: now.delivers(),
      points: view.values.points,
      ..Values::of_details(&details, separator)
    };
    let (fields, base, stop, dismissed) = (
      self.fields.clone(),
      self.base.clone(),
      self.team_stop.clone(),
      self.dismissed.clone(),
    );
    let separator: Arc<str> = Arc::from(separator);
    move || {
      fields.fill(&filled);
      base.set(Some(Base::of(&task, &separator)));
      stop.set(None);
      dismissed.set(true);
    }
  }
}

/// The warning callout of a change made first (`BrTjh`).
fn callout(title: Arc<str>, body: Arc<str>, now: Vec<Element>) -> Callout {
  now.into_iter().fold(
    Callout::new("task-form-team-conflict-callout", CalloutTone::Warning, title)
      .icon(Icon::GitCompare)
      .body(body),
    Callout::detail,
  )
}

/// The fields the person changed from what the edits were made on.
fn changed_fields(base: &Values, mine: &Values, separator: &str) -> Vec<Field> {
  let (base, mine) = (base.normalized(separator), mine.normalized(separator));
  let content = [
    (ContentField::Title, base.title != mine.title),
    (ContentField::Brief, base.brief != mine.brief),
    (ContentField::Criteria, base.criteria != mine.criteria),
    (ContentField::Pipeline, base.pipeline != mine.pipeline),
    (
      ContentField::Limits,
      base.budget != mine.budget || base.time_limit != mine.time_limit,
    ),
  ];
  let mut fields: Vec<Field> = content
    .into_iter()
    .filter(|(_, changed)| *changed)
    .map(|(field, _)| Field::Content(field))
    .collect();
  if base.priority != mine.priority {
    fields.push(Field::Priority);
  }
  fields
}

/// What `field` is in the task as it is now, on a line of its own.
fn now_line(ctx: &Ctx, field: Field, current: &Changed, pipelines: Option<&[PipelineChoice]>) -> Element {
  let (label, value) = match field {
    Field::Content(field) => (
      ctx.t(field_key(field)),
      value_text(ctx, field, current.task.details(), pipelines),
    ),
    Field::Priority => (
      ctx.t("task_form.fields.priority.label"),
      priority_word(ctx, current.task.organization().priority()),
    ),
  };
  let words = ctx.t_args(
    "task_form.team.changed.now",
    [("field", label.to_string()), ("value", value)],
  );
  Text::new(&words)
    .variant(ExtraTypography::Lead.role())
    .width(Dimension::Pct(100.0))
    .into()
}

fn priority_word(ctx: &Ctx, priority: Priority) -> String {
  ctx.t(priority_look(priority).1).to_string()
}

/// Any other refusal, in a danger notice: why, and that nothing (or only the changes before it) was saved.
fn refused(ctx: &Ctx, stop: &TeamStop, key: &str) -> Element {
  let title = if stop.saved.is_some() {
    "task_form.team.refused.titlePartly"
  } else {
    "task_form.team.refused.title"
  };
  let body = match &stop.stop {
    Stop::Unkept => "task_form.team.refused.unkept",
    Stop::Acted(Acted::Refused(AuthorityErrorCode::Forbidden)) => "task_form.team.refused.notAdmin",
    Stop::Acted(Acted::Refused(AuthorityErrorCode::DecisionRequired)) => "task_form.team.refused.decision",
    Stop::Acted(Acted::Refused(_) | Acted::Lifecycle(_)) => "task_form.team.refused.other",
    Stop::Acted(Acted::PipelineRefused(PipelineRefusal::NotFound)) => "task_form.team.refused.pipelineNotFound",
    Stop::Acted(Acted::PipelineRefused(PipelineRefusal::Archived)) => "task_form.team.refused.pipelineArchived",
    Stop::Acted(Acted::Unavailable) => "task_form.team.refused.unavailable",
    Stop::Acted(Acted::SignedOut) => "task_form.team.refused.signedOut",
    Stop::Acted(Acted::NeedsVersion(_)) => "summary.team.refused.needsVersion",
    Stop::Acted(Acted::Done | Acted::Changed(_) | Acted::Failed) => "task_form.team.refused.failed",
  };
  let body = match &stop.stop {
    Stop::Acted(Acted::NeedsVersion(required)) => ctx.t_args(body, [("version", required.as_str())]),
    _ => ctx.t(body),
  };
  Notice::new(
    "task-form-team-refused",
    NoticeTone::Danger,
    ctx.t_args(title, [("key", key)]),
  )
  .body(body)
  .into()
}
