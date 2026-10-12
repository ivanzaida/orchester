pub mod candidate;
mod state;
use std::sync::Arc;

use candidate::CandidateState;
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
use orchester_core::integration::local::{Checkout, EditorCandidate, LocalPath, LocalPhase, Placement, Resolution};
use orchester_workspace::tasks::local_integration::{ConflictView, IntegrationView, Preview};
pub(super) use state::EditorState;

use super::{
  LocalPage, TaskContext,
  files::preview,
  work::{Action, Outcome, Request, Work},
};
use crate::{
  navigation::guard::LeaveGuard,
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
  state: EditorState,
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
      state: page.resolver.clone(),
    },
  )
}

struct Resolver {
  selected: Signal<String>,
  editor: CandidateState,
  seeded: state::Seeded,
  leave: LeaveGuard,
  shared_dirty: Signal<bool>,
}

impl Component for Resolver {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let state = ctx.props::<Props>().state.clone();
    let editor = state.editor;
    let action = ctx.props::<Props>().work.clone();
    let completed = editor.clone();
    ctx.watch(&action.state(), move |state| {
      if !state.is_pending()
        && state.error.is_none()
        && let Some(Outcome::DraftSaved(captured) | Outcome::Resolved(captured)) = &state.data
      {
        completed.saved(captured);
      }
    });
    Self {
      selected: state.selected,
      editor,
      seeded: state.seeded,
      leave: state.leave,
      shared_dirty: ctx.props::<Props>().editor_dirty.clone(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    self.shared_dirty.set(self.editor.dirty());
    let files = &props.view.conflicts;
    let selected = self.selected.get();
    if self.editor.dirty() && !files.iter().any(|file| file.file.path.as_str() == selected) {
      return self.retained_notice(ctx, props.busy);
    }
    let Some(file) = files
      .iter()
      .find(|file| file.file.path.as_str() == selected)
      .or_else(|| files.first())
    else {
      if self.editor.dirty() {
        return self.retained_notice(ctx, props.busy);
      }
      return Element::from(Text::new(&ctx.t("local_integration.markers")).variant(TypographyStyle::Description));
    };
    let identity = (
      file.clone(),
      props.view.checkout.clone(),
      props.view.operation.as_ref().map(|op| op.version()),
    );
    if self.editor.dirty() && self.seeded.lock().as_ref() != Some(&identity) {
      return self.retained_notice(ctx, props.busy);
    }
    self.seed(file, identity);
    self.shared_dirty.set(self.editor.dirty());
    self.editor_body(ctx, &props, file, files)
  }
}

impl Resolver {
  fn editor_body(&self, ctx: &mut Ctx, props: &Props, file: &ConflictView, files: &[ConflictView]) -> Element {
    let picker = SelectField::new(
      "local-conflict-picker",
      ctx.t("local_integration.conflicts"),
      if self.editor.dirty() || props.busy {
        Signal::new(self.selected.get())
      } else {
        self.selected.clone()
      },
      files
        .iter()
        .map(|file| SelectOption::new(file.file.path.to_string(), file.file.path.as_str()))
        .collect(),
    );
    let picker = if self.editor.dirty() || props.busy {
      picker.locked()
    } else {
      picker
    };
    let picker = picker.render(ctx);
    let versions = self.versions(ctx, props, file);
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
    if file.draft_stale {
      result = result.child(Notice::new(
        "local-saved-draft-stale",
        NoticeTone::Warning,
        ctx.t("local_integration.error.draftStale"),
      ));
    }
    let sensitive_draft = props.view.editor_policy.sensitive(&self.editor.text.get());
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
        self.editor.text.clone(),
      )
      .text_area()
      .mono();
      let input = if props.busy { input.locked() } else { input };
      result = result.child(input.render(ctx));
      result = result.child(self.save_button(
        ctx,
        props,
        file,
        ("local-save-result", "local_integration.save"),
        (Resolution::Text(self.editor.text.get()), props.busy),
      ));
    }
    result = result
      .child(self.choice_controls(ctx, props, file))
      .child(Text::new(&ctx.t("local_integration.markers")).variant(TypographyStyle::Description));
    if self.leave.requested() || self.editor.dirty() {
      result = result.child(self.leave_controls(ctx, props, file, editable));
    }
    Element::from(result)
  }

  fn retained_notice(&self, ctx: &Ctx, busy: bool) -> Element {
    let editor = self.editor.clone();
    let props = ctx.props::<Props>();
    let work = props.work.clone();
    let refused = Request {
      task: props.task.clone(),
      action: Action::Refused,
      stop: props.stop.clone(),
    };
    let button = ActionButton::new(
      "local-discard-editor",
      ButtonKind::Danger,
      ctx.t("local_integration.discard"),
    );
    let button = if busy {
      button.disabled()
    } else {
      button.on_click(move || {
        if input_source() == InputSource::Synthetic {
          work.run_if_idle(refused.clone());
        } else {
          editor.discard();
        }
      })
    };
    Notice::new(
      "local-retained-candidate",
      NoticeTone::Warning,
      ctx.t("local_integration.unsaved"),
    )
    .body(ctx.t("local_integration.error.draftStale"))
    .action(button)
    .into()
  }
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

  fn seed(&self, file: &ConflictView, identity: (ConflictView, Checkout, Option<u64>)) {
    let mut seeded = self.seeded.lock();
    let path = file.file.path.to_string();
    if seeded.as_ref() != Some(&identity) && !self.editor.dirty() {
      let text = match &file.result {
        Preview::Text(text) => text.clone(),
        _ => String::new(),
      };
      let candidate = file
        .draft
        .clone()
        .unwrap_or_else(|| EditorCandidate::new(text, String::new(), Placement::LeaveUnstaged));
      self.editor.seed(&candidate);
      self.selected.set(path);
      *seeded = Some(identity);
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
      candidate: self.editor.current(),
      placement: if self.editor.placement.get() == "stage" {
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
      let mut request = request.clone();
      if input_source() == InputSource::Synthetic {
        request.action = Action::Refused;
      }
      request.stop.store(false, std::sync::atomic::Ordering::Release);
      work.run_if_idle(request);
    })
  }

  fn choice_controls(&self, ctx: &mut Ctx, props: &Props, file: &ConflictView) -> Element {
    let dirty = self.editor.text.get() != self.editor.baseline_text();
    let destination = FieldInput::new(
      "local-destination",
      ctx.t("local_integration.destination"),
      self.editor.destination.clone(),
    )
    .mono();
    let destination = if props.busy { destination.locked() } else { destination };
    let destination = if props.view.editor_policy.sensitive(&self.editor.destination.get()) {
      Element::from(Text::new(&ctx.t("local_integration.sensitiveDraft")).variant(TypographyStyle::Description))
    } else {
      destination.render(ctx)
    };
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
      .child(destination);
    choices = choices.with_children(self.destination_controls(ctx, props, file));
    if props
      .view
      .operation
      .as_ref()
      .is_some_and(|op| op.phase() == LocalPhase::RestoreConflicts)
    {
      let staging = SelectField::new(
        "local-staging-choice",
        ctx.t("local_integration.placement"),
        if props.busy {
          Signal::new(self.editor.placement.get())
        } else {
          self.editor.placement.clone()
        },
        vec![
          SelectOption::new(String::from("unstaged"), ctx.t("local_integration.leaveUnstaged")),
          SelectOption::new(String::from("stage"), ctx.t("local_integration.stage")),
        ],
      );
      let staging = if props.busy { staging.locked() } else { staging };
      choices = choices.child(staging.render(ctx));
    }
    choices.into()
  }

  fn destination_controls(&self, ctx: &Ctx, props: &Props, file: &ConflictView) -> Vec<Element> {
    let dirty = self.editor.text.get() != self.editor.baseline_text();
    if props.view.editor_policy.sensitive(&self.editor.destination.get()) {
      return Vec::new();
    }
    match LocalPath::new(&self.editor.destination.get()) {
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

  fn leave_controls(&self, ctx: &mut Ctx, props: &Props, file: &ConflictView, _editable: bool) -> Element {
    let request = Request {
      task: props.task.clone(),
      action: Action::SaveDraft {
        file: file.file.clone(),
        expected: file.expected.clone(),
        candidate: self.editor.current(),
        generation: props.view.operation.as_ref().map_or(0, |op| op.version()),
      },
      stop: props.stop.clone(),
    };
    super::draft_controls::inline(ctx, &props.work, &props.state, Some(request), props.busy, &props.task)
  }
}
