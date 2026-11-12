//! The panel of the pipeline being made (`Detail` `gWpRx` in P14 `Rszkt`): its head with the name, *Start from*, the
//! flow of what it starts from (unsaved, like any pipeline's: a starter's step with no profile is amber with *Choose
//! profile*, P14c `g09dL`), *Pipeline* and the bar with *Create v1*. The steps are an edit as a stored pipeline's are
//! ([`crate::screens::pipeline_hub::edit`]), started from the chosen start, which another start drops; a step opens in
//! the panel with ← back to the pipeline, and what was typed stays. A copy's steps are its source's latest version, not
//! drawn (P14b `sVNK4`).
//!
//! The panel tells the hub when something is typed (a name changed from the one it opened with, a description, a
//! change to the steps), which the question before leaving asks about; a pipeline with nothing typed goes without one.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::{Column, ScrollVertical, TextInputController},
  core::{ElementRef, Ref, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, PipelineEdit, Slug, StepKey},
};
use orchester_workspace::pipelines::{
  flow::FlowRead,
  local_edit::{blocking_steps, unpublishable},
  ready::step_problems,
};

use super::{
  Choice, Making, NewPipeline, bar,
  bar::Creating,
  create::{self, Ask, Create},
  details, head, start,
  words::{self, Lacks, Named},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{PipelinesFilter, PipelinesView, ProjectPage, Route},
  screens::{
    local_hub::leave::Leave,
    pipeline_hub::{
      OpenStep,
      edit::{Edit, Holding},
      flow,
      leave::{Edited, UnsavedSignal, clear_unsaved, mark_unsaved},
      model::{Entry, Listing},
      step,
    },
    pipelines::new_pipeline::starter,
    team_hub::panel::body,
  },
  theme::ExtraSpacing,
  ui::pressable::Press,
};

#[derive(Clone)]
pub(in crate::screens::pipeline_hub) struct Props {
  pub project: ShownProject,
  pub making: Making,
  pub new: NewPipeline,
  pub listing: Listing,
  /// What a press that leaves the panel does: it asks first while something is typed.
  pub leave: Leave,
  /// Where the hub hears that something is typed.
  pub unsaved: UnsavedSignal,
  /// Where the hub hears that *Create v1* is storing it: leaving waits for that.
  pub saving: Signal<bool>,
  /// The step, or the Deliver step, the route holds open.
  pub step: Option<OpenStep>,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (&self.project, &self.making, &self.listing, &self.leave, &self.step)
      == (&other.project, &other.making, &other.listing, &other.leave, &other.step)
  }
}
impl DevtoolsInspectable for Props {}

pub(in crate::screens::pipeline_hub) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  ctx.mount_keyed::<NewPanel>(&format!("pipeline-new-panel-{}", props.making.id), props)
}

/// The start an edit was made from: another start drops it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Based {
  choice: Choice,
  copy: String,
  /// The description the start came with, which the box shows until something is typed in it.
  description: String,
}

struct NewPanel {
  holding: Signal<Holding>,
  description: Signal<String>,
  based: Ref<Option<Based>>,
  create: Create,
  input: TextInputController,
  /// The name has had the keys once, as the panel opened.
  focused_once: Ref<bool>,
  focused: Signal<bool>,
  select: Signal<bool>,
  faulty: step::Faulty,
  reset: Signal<u64>,
  pick_agent: Signal<bool>,
  known: Signal<step::Paths>,
  gate: ElementRef,
  unsaved: UnsavedSignal,
  saving: Signal<bool>,
  id: ResourceId,
}

impl Component for NewPanel {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let project = props.project.open.project.id();
    let this = Self {
      holding: ctx.signal(Holding::default()),
      description: ctx.signal(String::new()),
      based: ctx.create_ref(None),
      create: create::action(ctx, props.project.open.repo_root.clone(), project),
      input: TextInputController::new(),
      focused_once: ctx.create_ref(false),
      focused: ctx.signal(false),
      select: ctx.signal(!props.making.first_name.is_empty()),
      faulty: ctx.signal(Vec::new()),
      reset: ctx.signal(0),
      pick_agent: ctx.signal(false),
      known: ctx.signal(step::Paths::default()),
      gate: ctx.element_ref(),
      unsaved: props.unsaved.clone(),
      saving: props.saving.clone(),
      id: props.making.id,
    };
    create::follow(ctx, &this.create, (&props.new, &props.unsaved));
    follow_copy(ctx, &props.new);
    this
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let made = self.made(ctx, &props);
    let state = self.state(ctx, &props, &made);
    let busy = self.create.state().get().is_pending() || props.making.created.is_some();
    mark(&self.saving, self.create.state().get().is_pending());
    self.tell_hub(&props, &made, &state);
    let body = match self.open_step(ctx, &props, &made) {
      Some(OpenStep::Step(key)) => self.step_body(ctx, &props, (&made, busy), &key),
      Some(OpenStep::Deliver) => self.deliver_body(ctx, &props, (&made, busy)),
      None => self.pipeline_body(ctx, &props, (&made, &state, busy)),
    };
    let refused = self
      .create
      .state()
      .get()
      .error
      .filter(|_| !busy)
      .map(|why| bar::refused(ctx, props.new.name.get_untracked().trim(), &why));
    let creating = self.creating(ctx, &props, (&made, &state, busy));
    let cancel = props.leave.press(Route::Project(
      props.project.open.project.id(),
      ProjectPage::pipelines(PipelinesView::new(PipelinesFilter::All, None)),
    ));
    frame(body, refused, bar::bar(ctx, creating, cancel))
  }
}

/// What the pipeline is made of in this draw.
struct Made {
  choice: Choice,
  /// The pipeline *Copy a pipeline* names, while it can be copied.
  source: Option<Entry>,
  starter: Pipeline,
  /// What the steps start from; `None` for a copy with nothing chosen.
  edit: Option<Edit>,
  current: PipelineEdit,
  read: Arc<FlowRead>,
}

/// How the pipeline stands: the name, and what keeps it from being created.
struct State {
  name: String,
  slug: Option<Slug>,
  taken_by: Option<Entry>,
  named: Named,
  /// Why *Create v1* is off; `None` once it can create.
  off: Option<Arc<str>>,
  typed: bool,
}

impl NewPanel {
  /// What the chosen start gives, and the edit made on it.
  fn made(&self, ctx: &Ctx, props: &Props) -> Made {
    let (choice, copy) = (props.new.choice.get(), props.new.copy.get());
    let source = copy
      .parse::<ResourceId>()
      .ok()
      .and_then(|id| props.listing.entry(id))
      .filter(|entry| entry.archived.is_none() && entry.body.is_some())
      .cloned();
    let assignables = props.listing.assignables();
    let starter = starter(ctx).pipeline(assignables);
    let base = match choice {
      Choice::Starter => Some(Arc::new(starter.clone())),
      Choice::Empty => Some(Arc::new(
        Pipeline::new(Vec::new()).expect("a pipeline without steps is a valid body"),
      )),
      Choice::Copy => source.as_ref().and_then(|entry| entry.body.clone()),
    };
    self.rebase(Based {
      choice,
      copy,
      description: base
        .as_ref()
        .map_or_else(String::new, |body| body.description().to_owned()),
    });
    let edit = base.map(|base| Edit::new(&self.holding, base));
    let current = edit.as_ref().map_or_else(
      || PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline without steps is a valid body")),
      Edit::current,
    );
    step::forget_gone(&self.faulty, &current);
    let read = Arc::new(FlowRead::of_edit(&current, assignables));
    Made {
      choice,
      source,
      starter,
      edit,
      current,
      read,
    }
  }

  /// Another start drops the edit made on the last one, and what its steps' fields kept; the description box takes the
  /// new start's description unless something was typed in it.
  fn rebase(&self, now: Based) {
    let before = self.based.get();
    if before.as_ref() == Some(&now) {
      return;
    }
    let typed = self.description.get_untracked();
    let untouched = before.as_ref().is_none_or(|before| before.description == typed);
    if before.is_some() {
      self.holding.set(Holding::default());
      self.faulty.set(Vec::new());
      self.reset.update(|resets| *resets += 1);
    }
    if untouched && typed != now.description {
      self.description.set(now.description.clone());
    }
    self.based.set(Some(now));
  }

  fn state(&self, ctx: &Ctx, props: &Props, made: &Made) -> State {
    let name = props.new.name.get();
    let slug = Slug::from_name(&name);
    let taken_by = slug
      .as_ref()
      .and_then(|slug| props.listing.with_slug(slug.as_str()))
      .cloned();
    let named = match (&slug, &taken_by) {
      _ if name.trim().is_empty() => Named::Empty,
      (None, _) => Named::NoSlug,
      (Some(_), Some(_)) => Named::Taken,
      (Some(_), None) => Named::Free,
    };
    let problems = blocking_steps(&made.read);
    let unpublishable = unpublishable(&made.current, &made.read, props.listing.assignables());
    let lacks = match (&made.edit, &unpublishable) {
      (None, _) => Some(Lacks::Copy),
      (Some(_), Some(why)) => Some(Lacks::Steps(why, &problems)),
      (Some(_), None) => None,
    };
    let off = step::fix_words(ctx, &self.faulty, &made.current)
      .map(Arc::from)
      .or_else(|| words::off(ctx, named, lacks));
    let description = self.description.get();
    let typed = name != props.making.first_name
      || self.based.get().is_some_and(|based| based.description != description)
      || made.edit.as_ref().is_some_and(Edit::is_changed)
      || step::holds_invalid(&self.faulty, &made.current);
    State {
      name,
      slug,
      taken_by,
      named,
      off,
      typed,
    }
  }

  /// Tells the hub whether something is typed, which the question before leaving asks about, and how many steps the
  /// pipeline has, which its row says. Nothing is left to ask about once *Create v1* stored it.
  fn tell_hub(&self, props: &Props, made: &Made, state: &State) {
    let edited = (state.typed && props.making.created.is_none()).then(|| Edited {
      pipeline: props.making.id,
      steps_not_ready: step_problems(&made.read).len(),
      save_off: state.off.is_some(),
    });
    mark_unsaved(&props.unsaved, edited);
    let steps = made.current.steps().len();
    if props.new.steps.get_untracked() != steps {
      props.new.steps.set(steps);
    }
  }

  /// The step or the Deliver step the route holds open, when the pipeline has it (a copy's too, whose steps are not
  /// drawn); a route that names one it has not (any more, as after another start) shows
  /// the pipeline and is corrected to say so.
  fn open_step(&self, ctx: &mut Ctx, props: &Props, made: &Made) -> Option<OpenStep> {
    let named = props.step.clone()?;
    let held = made.edit.is_some()
      && match &named {
        OpenStep::Step(key) => made.current.step(key).is_some(),
        OpenStep::Deliver => made.current.deliver().is_some(),
      };
    if held {
      return Some(named);
    }
    let route = ProjectPage::pipelines(PipelinesView::new(PipelinesFilter::All, Some(props.making.id)));
    Navigation::of(ctx).replace(Route::Project(props.project.open.project.id(), route));
    None
  }

  /// The pipeline's own body: the head, *Start from*, the flow of a starter or an empty pipeline, and *Pipeline*.
  fn pipeline_body(&self, ctx: &mut Ctx, props: &Props, (made, state, busy): (&Made, &State, bool)) -> Column {
    if !self.focused_once.get() {
      self.focused_once.set(true);
      self.input.focus(ctx);
    }
    let head = head::head(
      ctx,
      &head::Head {
        what: words::what(ctx, made.source.as_ref().filter(|_| made.choice == Choice::Copy)),
        name: &props.new.name,
        input: &self.input,
        focused: &self.focused,
        select: &self.select,
        taken: state.named == Named::Taken,
        locked: busy,
      },
    );
    let copies: Vec<&Entry> = props.listing.shown(PipelinesFilter::All).collect();
    let start = start::group(
      ctx,
      &start::Start {
        choice: &props.new.choice,
        copy: &props.new.copy,
        starter: &made.starter,
        starter_gates: gates(&FlowRead::of_edit(
          &PipelineEdit::new(made.starter.clone()),
          props.listing.assignables(),
        )),
        copies: &copies,
        source: made.source.as_ref(),
        locked: busy,
      },
    );
    let steps = made.edit.as_ref().filter(|_| made.choice != Choice::Copy).map(|edit| {
      flow::section(
        ctx,
        (&props.project, props.making.id, made.current.steps().len()),
        &made.read,
        (edit, &made.current),
        (busy, &self.pick_agent),
      )
    });
    let details = details::group(
      ctx,
      &details::Details {
        project: props.project.open.project.id(),
        description: &self.description,
        slug: state.slug.as_ref().map(Slug::as_str),
        taken_by: state.taken_by.as_ref(),
        leave: &props.leave,
        locked: busy,
      },
    );
    body()
      .spacing(ExtraSpacing::Inset.role())
      .child(head)
      .child(start)
      .with_children(steps)
      .child(details)
  }

  /// A step of the pipeline opened in the panel, as a stored pipeline's opens.
  fn step_body(&self, ctx: &mut Ctx, props: &Props, (made, busy): (&Made, bool), key: &StepKey) -> Column {
    let Some(edit) = made.edit.as_ref() else {
      return body();
    };
    let entry = entry_of(ctx, props, made);
    body().spacing(ExtraSpacing::Inset.role()).with_children(step::parts(
      ctx,
      &step::Open {
        project: &props.project,
        entry: &entry,
        key,
        edit,
        current: &made.current,
        read: &made.read,
        assignables: props.listing.assignables(),
        locked: busy,
        epoch: self.reset.get(),
        faulty: &self.faulty,
        pick_agent: &self.pick_agent,
        known: &self.known,
        gate: &self.gate,
      },
    ))
  }

  /// The Deliver step of the pipeline opened in the panel, as a stored pipeline's opens (P15 `nPaMj`).
  fn deliver_body(&self, ctx: &mut Ctx, props: &Props, (made, busy): (&Made, bool)) -> Column {
    let Some(edit) = made.edit.as_ref() else {
      return body();
    };
    let entry = entry_of(ctx, props, made);
    body()
      .spacing(ExtraSpacing::Inset.role())
      .with_children(step::deliver_parts(
        ctx,
        &step::DeliverOpen {
          project: &props.project,
          entry: &entry,
          edit,
          current: &made.current,
          read: &made.read,
          assignables: props.listing.assignables(),
          locked: busy,
          epoch: self.reset.get(),
        },
      ))
  }

  /// What *Create v1* does now: it stores the pipeline once nothing keeps it off.
  fn creating(&self, ctx: &Ctx, props: &Props, (made, state, busy): (&Made, &State, bool)) -> Creating {
    let says = words::creates(ctx, &state.name);
    if busy {
      return Creating::Busy(says);
    }
    if let Some(why) = state.off.clone() {
      return Creating::Off(why);
    }
    let Some(edit) = made.edit.clone() else {
      return Creating::Off(ctx.t("pipeline_new.bar.copy"));
    };
    let (create, name, description) = (self.create.clone(), props.new.name.clone(), self.description.clone());
    Creating::Ready(
      Press::run(move || {
        // What is offered has a valid order of steps (`unpublishable` checks it), so this always holds.
        let Ok(body) = edit.current_now().validate() else {
          return;
        };
        create.run(Ask {
          name: name.get_untracked().trim().to_owned(),
          body: body.with_description(description.get_untracked()),
        });
      }),
      says,
    )
  }
}

impl Drop for NewPanel {
  fn drop(&mut self) {
    clear_unsaved(&self.unsaved, self.id);
    mark(&self.saving, false);
  }
}

/// Tells the hub whether *Create v1* is storing the pipeline; set only when it differs.
fn mark(saving: &Signal<bool>, now: bool) {
  if saving.get_untracked() != now {
    saving.set(now);
  }
}

/// Choosing a pipeline in *Copy a pipeline*'s select chooses that row.
fn follow_copy(ctx: &mut Ctx, new: &NewPipeline) {
  let choice = new.choice.clone();
  ctx.watch(&new.copy, move |copy: &String| {
    if !copy.is_empty() && choice.get_untracked() != Choice::Copy {
      choice.set(Choice::Copy);
    }
  });
}

/// How many approval gates `read`'s flow has.
fn gates(read: &FlowRead) -> usize {
  read
    .flow
    .as_ref()
    .map_or(0, |flow| flow.stages.iter().map(|stage| stage.gates.len()).sum())
}

/// The pipeline as an open step reads it: its id in the hub's routes and the name it shows.
fn entry_of(ctx: &Ctx, props: &Props, made: &Made) -> Entry {
  let name = props
    .new
    .name_of(ctx, props.making.id)
    .unwrap_or_else(|| ctx.t("pipeline_new.untitled").to_string());
  Entry {
    id: props.making.id,
    name: name.into(),
    slug: Arc::from(""),
    step_count: made.current.steps().len(),
    used_by: 0,
    number: None,
    published_at: None,
    group: orchester_workspace::readiness::Group::Ready,
    standing: None,
    undecided: Vec::new(),
    archived: None,
    flow: Some(made.read.clone()),
    body: None,
    published: None,
  }
}

/// The panel: `body` scrolls over what a refused create says, and the bar.
fn frame(body: Column, refused: Option<Element>, bar: Element) -> Element {
  let scroll = ScrollVertical::new(body)
    .id("pipeline-new-scroll")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0));
  Column::new()
    .id("pipeline-new-panel")
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Pct(100.0))
    .child(scroll)
    .with_children(refused)
    .child(bar)
    .into()
}
