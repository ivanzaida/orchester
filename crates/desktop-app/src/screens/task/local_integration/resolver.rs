use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::{InputSource, input_source},
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, SelectOption, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::integration::local::{LocalPath, LocalPhase, Placement, Resolution};
use orchester_workspace::tasks::local_integration::{ConflictView, IntegrationView, Preview};

use super::{
  LocalPage, TaskContext,
  files::preview,
  work::{Action, Request, Work},
};
use crate::{
  navigation::{Navigation, guard::LeaveGuard},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, SelectField},
    notice::{Notice, NoticeTone},
  },
};

#[derive(Clone)]
struct Props {
  task: TaskContext,
  view: Arc<IntegrationView>,
  work: Work,
  stop: Arc<std::sync::atomic::AtomicBool>,
  busy: bool,
  narrow: bool,
  editor_dirty: Signal<bool>,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.task == other.task && self.view == other.view && self.busy == other.busy && self.narrow == other.narrow
  }
}
impl DevtoolsInspectable for Props {}

pub(super) fn mount(
  ctx: &mut Ctx,
  page: &LocalPage,
  task: &TaskContext,
  view: &IntegrationView,
  busy: bool,
  narrow: bool,
) -> Element {
  ctx.mount_keyed::<Resolver>(
    "local-resolver",
    Props {
      task: task.clone(),
      view: Arc::new(view.clone()),
      work: page.work.clone(),
      stop: page.stop.clone(),
      busy,
      narrow,
      editor_dirty: page.editor_dirty.clone(),
    },
  )
}

struct Resolver {
  selected: Signal<String>,
  draft: Signal<String>,
  saved: Signal<String>,
  destination: Signal<String>,
  placement: Signal<String>,
  seeded: parking_lot::Mutex<String>,
  leave: LeaveGuard,
  leave_after_save: Signal<bool>,
  shared_dirty: Signal<bool>,
}

impl Component for Resolver {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let draft = Signal::new(String::new());
    let saved = Signal::new(String::new());
    let (drafts, original) = (draft.clone(), saved.clone());
    let leave = Navigation::of(ctx).guard(move || drafts.get_untracked() != original.get_untracked());
    let action = ctx.props::<Props>().work.clone();
    let (saved_after, draft_after) = (saved.clone(), draft.clone());
    ctx.watch(&action.state(), move |state| {
      if !state.is_pending() && state.error.is_none() && state.data.is_some() {
        saved_after.set(draft_after.get_untracked());
      }
    });
    Self {
      selected: ctx.signal(String::new()),
      draft,
      saved,
      destination: Signal::new(String::new()),
      placement: ctx.signal(String::from("unstaged")),
      seeded: parking_lot::Mutex::new(String::new()),
      leave,
      leave_after_save: ctx.signal(false),
      shared_dirty: ctx.props::<Props>().editor_dirty.clone(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    self.shared_dirty.set(self.draft.get() != self.saved.get());
    let files = &props.view.conflicts;
    let selected = self.selected.get();
    let Some(file) = files
      .iter()
      .find(|file| file.file.path.as_str() == selected)
      .or_else(|| files.first())
    else {
      return Element::from(Text::new(&ctx.t("local_integration.markers")).variant(TypographyStyle::Description));
    };
    self.seed(file);
    let picker = SelectField::new(
      "local-conflict-picker",
      ctx.t("local_integration.conflicts"),
      self.selected.clone(),
      files
        .iter()
        .map(|file| SelectOption::new(file.file.path.to_string(), file.file.path.as_str()))
        .collect(),
    );
    let picker = if self.draft.get() != self.saved.get() || props.busy {
      picker.locked()
    } else {
      picker
    };
    let picker = picker.render(ctx);
    let versions = self.versions(ctx, &props, file);
    let mut result = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(picker)
      .child(Text::new(&file.file.path.to_string()).variant(TypographyStyle::Mono))
      .child(preview(
        ctx,
        "local-conflict-ancestor",
        "local_integration.ancestor",
        &file.ancestor,
      ))
      .child(versions);
    let sensitive_draft = props.view.editor_policy.sensitive(&self.draft.get());
    let editable = !sensitive_draft
      && matches!(file.result, Preview::Text(_) | Preview::Absent)
      && matches!(file.ancestor, Preview::Text(_) | Preview::Absent)
      && matches!(file.current, Preview::Text(_) | Preview::Absent)
      && matches!(file.incoming, Preview::Text(_) | Preview::Absent);
    if sensitive_draft {
      result = result.child(Notice::new(
        "local-sensitive-draft",
        NoticeTone::Warning,
        ctx.t("local_integration.sensitiveDraft"),
      ));
    }
    if editable {
      let input = FieldInput::new(
        "local-result-editor",
        ctx.t("local_integration.result"),
        self.draft.clone(),
      )
      .text_area()
      .mono();
      let input = if props.busy { input.locked() } else { input };
      result = result.child(input.render(ctx));
      result = result.child(self.save_button(
        ctx,
        &props,
        file,
        ("local-save-result", "local_integration.save"),
        (Resolution::Text(self.draft.get()), props.busy),
      ));
    }
    result = result
      .child(self.choice_controls(ctx, &props, file))
      .child(Text::new(&ctx.t("local_integration.markers")).variant(TypographyStyle::Description));
    if self.leave.requested() || self.draft.get() != self.saved.get() {
      result = result.child(self.leave_controls(ctx, &props, file, editable));
    }
    self.finish_saved_leave(ctx, &props);
    Element::from(result)
  }
}

impl Resolver {
  fn versions(&self, ctx: &mut Ctx, props: &Props, file: &ConflictView) -> Element {
    let current = preview(
      ctx,
      "local-conflict-current",
      "local_integration.current",
      &file.current,
    );
    let incoming = preview(
      ctx,
      "local-conflict-incoming",
      "local_integration.incoming",
      &file.incoming,
    );
    if props.narrow {
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .child(current)
        .child(incoming)
        .into()
    } else {
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .child(Column::new().flex(1.0).child(current))
        .child(Column::new().flex(1.0).child(incoming))
        .into()
    }
  }

  fn seed(&self, file: &ConflictView) {
    let mut seeded = self.seeded.lock();
    let path = file.file.path.to_string();
    if *seeded != path && self.draft.get_untracked() == self.saved.get_untracked() {
      let text = match &file.result {
        Preview::Text(text) => text.clone(),
        _ => String::new(),
      };
      self.draft.set(text.clone());
      self.saved.set(text);
      self.selected.set(path.clone());
      self.destination.set(String::new());
      *seeded = path;
    }
  }

  fn save_button(
    &self,
    ctx: &Ctx,
    props: &Props,
    file: &ConflictView,
    (id, label): (&str, &str),
    (resolution, disabled): (Resolution, bool),
  ) -> ActionButton {
    let action = Action::Save {
      file: file.file.clone(),
      expected: file.expected.clone(),
      resolution,
      placement: if self.placement.get() == "stage" {
        Placement::Stage
      } else {
        Placement::LeaveUnstaged
      },
      restore: props
        .view
        .operation
        .as_ref()
        .is_some_and(|op| op.phase() == LocalPhase::RestoreConflicts),
    };
    let request = Request {
      task: props.task.clone(),
      action,
      stop: props.stop.clone(),
    };
    let work = props.work.clone();
    let button = ActionButton::new(id, ButtonKind::Secondary, ctx.t(label));
    if disabled {
      return button.disabled();
    }
    button.on_click(move || {
      if input_source() == InputSource::Synthetic {
        return;
      }
      request.stop.store(false, std::sync::atomic::Ordering::Release);
      work.run_if_idle(request.clone());
    })
  }

  fn choice_controls(&self, ctx: &mut Ctx, props: &Props, file: &ConflictView) -> Element {
    let dirty = self.draft.get() != self.saved.get();
    let mut choices = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(self.save_button(
        ctx,
        props,
        file,
        ("local-use-current", "local_integration.useCurrent"),
        (Resolution::Current, props.busy || dirty),
      ))
      .child(self.save_button(
        ctx,
        props,
        file,
        ("local-use-incoming", "local_integration.useIncoming"),
        (Resolution::Incoming, props.busy || dirty),
      ))
      .child(self.save_button(
        ctx,
        props,
        file,
        ("local-delete", "local_integration.delete"),
        (Resolution::Delete, props.busy || dirty),
      ))
      .child(
        FieldInput::new(
          "local-destination",
          ctx.t("local_integration.destination"),
          self.destination.clone(),
        )
        .mono()
        .render(ctx),
      );
    choices = choices.with_children(self.destination_controls(ctx, props, file));
    if props
      .view
      .operation
      .as_ref()
      .is_some_and(|op| op.phase() == LocalPhase::RestoreConflicts)
    {
      choices = choices.child(
        SelectField::new(
          "local-staging-choice",
          ctx.t("local_integration.placement"),
          self.placement.clone(),
          vec![
            SelectOption::new(String::from("unstaged"), ctx.t("local_integration.leaveUnstaged")),
            SelectOption::new(String::from("stage"), ctx.t("local_integration.stage")),
          ],
        )
        .render(ctx),
      );
    }
    choices.into()
  }

  fn destination_controls(&self, ctx: &Ctx, props: &Props, file: &ConflictView) -> Vec<Element> {
    let dirty = self.draft.get() != self.saved.get();
    match LocalPath::new(&self.destination.get()) {
      Ok(destination) => vec![
        self
          .save_button(
            ctx,
            props,
            file,
            ("local-keep-both", "local_integration.keepBoth"),
            (Resolution::KeepBoth(destination.clone()), props.busy || dirty),
          )
          .into(),
        self
          .save_button(
            ctx,
            props,
            file,
            ("local-save-at-path", "local_integration.saveAtPath"),
            (
              Resolution::AtPath {
                incoming: true,
                destination,
              },
              props.busy || dirty,
            ),
          )
          .into(),
      ],
      Err(_) => vec![
        ActionButton::new(
          "local-keep-both",
          ButtonKind::Secondary,
          ctx.t("local_integration.keepBoth"),
        )
        .disabled()
        .into(),
        ActionButton::new(
          "local-save-at-path",
          ButtonKind::Secondary,
          ctx.t("local_integration.saveAtPath"),
        )
        .disabled()
        .into(),
      ],
    }
  }

  fn leave_controls(&self, ctx: &mut Ctx, props: &Props, file: &ConflictView, editable: bool) -> Element {
    let leave = self.leave.clone();
    let keep = self.leave.clone();
    let navigation = Navigation::of(ctx);
    let draft = self.draft.clone();
    let saved = self.saved.clone();
    let flag = self.leave_after_save.clone();
    let save = self.save_button(
      ctx,
      props,
      file,
      ("local-save-pause", "local_integration.savePause"),
      (Resolution::Text(self.draft.get()), props.busy || !editable),
    );
    Notice::new("local-unsaved", NoticeTone::Warning, ctx.t("local_integration.unsaved"))
      .body(ctx.t("local_integration.unsavedBody"))
      .action(save.on_click({
        let work = props.work.clone();
        let request = Request {
          task: props.task.clone(),
          action: Action::Save {
            file: file.file.clone(),
            expected: file.expected.clone(),
            resolution: Resolution::Text(self.draft.get()),
            placement: if self.placement.get() == "stage" {
              Placement::Stage
            } else {
              Placement::LeaveUnstaged
            },
            restore: props
              .view
              .operation
              .as_ref()
              .is_some_and(|op| op.phase() == LocalPhase::RestoreConflicts),
          },
          stop: props.stop.clone(),
        };
        move || {
          if input_source() != InputSource::Synthetic {
            flag.set(true);
            request.stop.store(true, std::sync::atomic::Ordering::Release);
            work.run_if_idle(request.clone());
          }
        }
      }))
      .action(
        ActionButton::new(
          "local-keep-editing",
          ButtonKind::Secondary,
          ctx.t("local_integration.keepEditing"),
        )
        .on_click(move || keep.keep_editing()),
      )
      .action(
        ActionButton::new(
          "local-discard-editor",
          ButtonKind::Danger,
          ctx.t("local_integration.discard"),
        )
        .on_click(move || {
          if input_source() == InputSource::Synthetic {
            return;
          }
          draft.set(saved.get_untracked());
          leave.leave(&navigation);
        }),
      )
      .into()
  }

  fn finish_saved_leave(&self, ctx: &mut Ctx, props: &Props) {
    if !self.leave_after_save.get() {
      return;
    }
    let state = props.work.state().get();
    if !state.is_pending() {
      self.leave_after_save.set(false);
      if state.error.is_none() && state.data.is_some() {
        self.saved.set(self.draft.get_untracked());
        self.leave.leave(&Navigation::of(ctx));
      }
    }
  }
}
impl Drop for Resolver {
  fn drop(&mut self) {
    self.shared_dirty.set(false);
  }
}
