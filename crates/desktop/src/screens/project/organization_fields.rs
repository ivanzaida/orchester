use std::path::PathBuf;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  id::TaskId,
  task::organization::{BlockingTask, DependencyGate, OrganizationError, Priority, TaskOrganization, WorkflowStatus},
};

use super::{
  run_blockers::RunBlockers,
  tag_picker::{TagPicker, TagPickerProps},
};
use crate::{
  tasks::organization::{
    OrganizationValue, PickerAppearance, PriorityChoice, WorkflowChoice, priority_select, workflow_select,
  },
  theme::ExtraSpacing,
  ui::{
    button::ActionButton,
    field::{FieldNote, field, locked_input, text_input},
    icon::Icon,
    menu_option::menu_option,
  },
};

// tUwII: the search and its results.
const RESULTS_WIDTH: f32 = 420.0;

#[derive(Clone)]
pub struct OrganizationDraft {
  pub status: Signal<WorkflowChoice>,
  pub priority: Signal<PriorityChoice>,
  pub blocked: Signal<bool>,
  pub reason: Signal<String>,
  pub value: Signal<OrganizationValue>,
  /// What the form loaded, or saved last: a save keeps what changed elsewhere since and writes
  /// only what the person changed from it.
  pub loaded: Signal<OrganizationValue>,
  pub ready: Signal<bool>,
  /// What blocks Run, shown beside the run action (`ZLhiM`).
  pub blockers: Signal<RunBlockers>,
}

impl OrganizationDraft {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      status: ctx.signal(WorkflowChoice(WorkflowStatus::Backlog)),
      priority: ctx.signal(PriorityChoice(Priority::None)),
      blocked: ctx.signal(false),
      reason: ctx.signal(String::new()),
      value: ctx.signal(OrganizationValue::default()),
      loaded: ctx.signal(OrganizationValue::default()),
      ready: ctx.signal(true),
      blockers: ctx.signal(RunBlockers::default()),
    }
  }
  pub fn seed(&self, value: &TaskOrganization) {
    self.status.set(WorkflowChoice(value.status()));
    self.priority.set(PriorityChoice(value.priority()));
    self.blocked.set(value.blocked_reason().is_some());
    self.reason.set(value.blocked_reason().unwrap_or_default().to_owned());
    self.value.set(OrganizationValue(value.clone()));
    self.loaded.set(OrganizationValue(value.clone()));
  }

  /// What the form loaded and what it has now, for a save; `None` while the fields are invalid,
  /// which they explain.
  pub fn change(&self) -> Option<(TaskOrganization, TaskOrganization)> {
    Some((self.loaded.get().0, self.snapshot().ok()?))
  }
  pub fn snapshot(&self) -> Result<TaskOrganization, OrganizationError> {
    let value = self.value.get().0;
    TaskOrganization::new(
      self.status.get().0,
      self.priority.get().0,
      self.blocked.get().then(|| self.reason.get()),
      value.dependencies().to_vec(),
      value.tags().to_vec(),
    )
  }
}

#[derive(Clone)]
pub struct OrganizationFieldsProps {
  pub draft: OrganizationDraft,
  pub root: PathBuf,
  pub task: Option<TaskId>,
  /// The form is locked (a queued task awaiting its first run, or Create & run under way): the
  /// fields show the stored values and cannot change.
  pub locked: bool,
}
impl PartialEq for OrganizationFieldsProps {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root && self.task == other.task && self.locked == other.locked
  }
}
impl lurq::app::component::DevtoolsInspectable for OrganizationFieldsProps {}

pub struct OrganizationFields;
impl Component for OrganizationFields {
  type Props = OrganizationFieldsProps;
  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<OrganizationFieldsProps>().clone();
    let draft = &props.draft;
    let blocked = draft.blocked.get();
    let flip = draft.blocked.clone();
    let reason = draft.reason.clone();
    let note = if blocked && draft.reason.get().trim().is_empty() {
      FieldNote::Error(ctx.t("tasks.blocked.reasonRequired"))
    } else {
      FieldNote::None
    };
    let mut content = Column::new()
      .spacing(SpacingSize::Lg)
      .child(organization_pickers(ctx, draft, !props.locked));
    let tags = ctx.mount_keyed::<TagPicker>(
      "form-tags",
      TagPickerProps {
        root: props.root.clone(),
        inline: false,
        appearance: PickerAppearance::Field,
        selected: draft.value.clone(),
        enabled: !props.locked,
      },
    );
    content = content
      .child(field(&ctx.t("tasks.tags.label"), tags, FieldNote::None))
      .child(blocked_control(ctx, blocked, flip, !props.locked));
    if blocked {
      let input = text_input(
        ctx,
        "task-blocked-reason",
        reason,
        draft.reason.get().trim().is_empty(),
        TypographyStyle::Body,
      )
      .multiline()
      .padding(ExtraSpacing::TextArea.role())
      .height(Dimension::Px(BLOCKED_REASON_HEIGHT));
      let control: Element = locked_input(input, props.locked).into();
      content = content.child(field(&ctx.t("tasks.blocked.reason"), control, note));
    }
    if blocked || !draft.value.get().0.dependencies().is_empty() {
      content = content.child(ctx.mount_keyed::<DependencyFields>(
        "blocking-links",
        DependencyProps {
          root: props.root,
          task: props.task,
          locked: props.locked,
          value: draft.value.clone(),
          ready: draft.ready.clone(),
          blockers: draft.blockers.clone(),
          reason: draft.reason.clone(),
          blocked: draft.blocked.clone(),
        },
      ));
    } else {
      draft.ready.set(true);
      if draft.blockers.get_untracked() != RunBlockers::default() {
        draft.blockers.set(RunBlockers::default());
      }
    }
    content
  }
}

fn organization_pickers(ctx: &Ctx, draft: &OrganizationDraft, enabled: bool) -> Element {
  Row::new()
    .spacing(SpacingSize::Lg)
    .child(Column::new().flex(1.0).child(field(
      &ctx.t("tasks.workflow.label"),
      workflow_select(
        ctx,
        draft.status.clone(),
        "task-workflow-field",
        enabled,
        PickerAppearance::Field,
      ),
      FieldNote::None,
    )))
    .child(Column::new().flex(1.0).child(field(
      &ctx.t("tasks.priority.label"),
      priority_select(
        ctx,
        draft.priority.clone(),
        "task-priority-field",
        enabled,
        PickerAppearance::Field,
      ),
      FieldNote::None,
    )))
    .into()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DependencyChoice(Option<TaskId>);
impl lurq::app::component::DevtoolsInspectable for DependencyChoice {}
#[derive(Clone)]
struct DependencyProps {
  root: PathBuf,
  task: Option<TaskId>,
  locked: bool,
  value: Signal<OrganizationValue>,
  ready: Signal<bool>,
  blockers: Signal<RunBlockers>,
  reason: Signal<String>,
  blocked: Signal<bool>,
}
impl PartialEq for DependencyProps {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root && self.task == other.task && self.locked == other.locked
  }
}
impl lurq::app::component::DevtoolsInspectable for DependencyProps {}
struct DependencyFields {
  choice: Signal<DependencyChoice>,
  query: Signal<String>,
  /// Add blocking task opened the search (`ZLhiM` closed, `tUwII` open).
  picking: Signal<bool>,
}
impl Component for DependencyFields {
  type Props = DependencyProps;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      choice: ctx.signal(DependencyChoice(None)),
      query: ctx.signal(String::new()),
      picking: ctx.signal(false),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DependencyProps>().clone();
    let shown = ctx
      .use_context::<super::pages::Shown>()
      .expect("shell provides project data");
    let programs = ctx
      .use_context::<crate::programs::Programs>()
      .expect("the root provides Programs before mounting screens");
    let load = ctx.future((props.root.clone(), shown.revision), move |(root, _)| {
      let programs = programs.clone();
      async move {
        crate::background::run(move || crate::tasks::data::ProjectTasks::load(&root, &programs))
          .await?
          .map_err(|error| crate::messages::store_error(&error))
      }
    });
    let state = load.state().get();
    let mut column = Column::new()
      .spacing(SpacingSize::Sm)
      .child(Text::new(&ctx.t("tasks.blocked.links")).variant(TypographyStyle::Caption));
    if let Some(error) = state.error {
      props.ready.set(false);
      return column.child(crate::ui::field::field_error(&crate::ui::message_text(ctx, &error)));
    }
    let Some(tasks) = state.data else {
      props.ready.set(props.value.get().0.dependencies().is_empty());
      return column.child(Text::new(&ctx.t("tasks.tags.loading")).variant(TypographyStyle::Caption));
    };
    let organization = props.value.get().0;
    let unfinished = organization.unfinished(|link| {
      tasks
        .phases
        .get(&link.task)
        .and_then(|latest| latest.phase())
        .is_some_and(|phase| link.gate.is_completed(phase))
    });
    props.ready.set(unfinished.is_empty());
    let blockers = RunBlockers::of(&tasks, &organization, &unfinished);
    if props.blockers.get_untracked() != blockers {
      props.blockers.set(blockers);
    }
    column = column.child(readiness_note(
      ctx,
      &tasks,
      &unfinished,
      &if props.blocked.get() {
        props.reason.get()
      } else {
        String::new()
      },
    ));
    column = self.link_rows(ctx, &props, &tasks, column);
    if props.locked {
      return column;
    }
    column.child(self.add_control(ctx, &props, &tasks))
  }
}

impl DependencyFields {
  fn link_rows(
    &self,
    ctx: &mut Ctx,
    props: &DependencyProps,
    tasks: &crate::tasks::data::ProjectTasks,
    mut column: Column,
  ) -> Column {
    let organization = props.value.get().0;
    for link in organization.dependencies() {
      let id = link.task;
      let blocker = tasks.tasks.iter().find(|task| task.id() == id);
      let label = blocker
        .map(|task| {
          ctx.t_args(
            "tasks.blocked.linkLabel",
            [
              ("key", task.key().to_string().as_str()),
              ("title", task.details().title.as_str()),
            ],
          )
        })
        .unwrap_or_else(|| id.to_string().into());
      let nav = ctx
        .use_context::<super::ProjectNav>()
        .expect("shell provides navigation");
      let value = props.value.clone();
      column = column.child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .child(
            ActionButton::new(label)
              .icon(Icon::Link)
              .in_form()
              .link(nav.link(crate::navigation::route::Page::task(id)))
              .render(ctx),
          )
          .with_children(
            blocker.map(|task| crate::tasks::organization::workflow_label(ctx, task.organization().status())),
          )
          .child(
            ActionButton::new(ctx.t("tasks.blocked.removeLink"))
              .icon(Icon::X)
              .in_form()
              .on_click(move || {
                value.update(|value| {
                  let old = &value.0;
                  value.0 = TaskOrganization::new(
                    old.status(),
                    old.priority(),
                    old.blocked_reason().map(str::to_owned),
                    old
                      .dependencies()
                      .iter()
                      .filter(|link| link.task != id)
                      .cloned()
                      .collect(),
                    old.tags().to_vec(),
                  )
                  .expect("removing a dependency preserves organization invariants");
                });
              })
              .enabled(!props.locked)
              .render(ctx),
          ),
      );
    }
    column
  }

  fn creates_cycle(&self, props: &DependencyProps, tasks: &crate::tasks::data::ProjectTasks) -> bool {
    self.choice.get().0.zip(props.task).is_some_and(|(id, task)| {
      let graph = tasks
        .tasks
        .iter()
        .map(|task| (task.id(), task.organization().clone()))
        .collect();
      TaskOrganization::new(
        WorkflowStatus::Backlog,
        Priority::None,
        None,
        vec![BlockingTask {
          task: id,
          gate: DependencyGate::Merge,
        }],
        vec![],
      )
      .expect("a prospective link has valid local fields")
      .check_dependencies(task, &graph)
      .is_err()
    })
  }

  /// The tasks matching the search, each with a check once chosen (`tUwII`); the current task is
  /// named apart, since it cannot block itself.
  /// The tasks the search offers: matching it, neither the current task nor one already linked.
  fn offered<'a>(
    &self,
    props: &DependencyProps,
    tasks: &'a crate::tasks::data::ProjectTasks,
  ) -> Vec<&'a orchester_core::task::Task> {
    let linked = props.value.get().0;
    tasks
      .tasks
      .iter()
      .filter(|task| self.matches(task))
      .filter(|task| Some(task.id()) != props.task)
      .filter(|task| !linked.dependencies().iter().any(|link| link.task == task.id()))
      .collect()
  }

  fn matches(&self, task: &orchester_core::task::Task) -> bool {
    let query = self.query.get().trim().to_lowercase();
    !query.is_empty()
      && (task.details().title.as_str().to_lowercase().contains(&query)
        || task.key().to_string().to_lowercase().contains(&query))
  }

  fn results(&self, ctx: &Ctx, props: &DependencyProps, tasks: &crate::tasks::data::ProjectTasks) -> Column {
    let matches = |task: &&orchester_core::task::Task| self.matches(task);
    let chosen = self.choice.get().0;
    let options: Vec<Element> = self
      .offered(props, tasks)
      .into_iter()
      .map(|task| {
        let label = ctx.t_args(
          "tasks.blocked.pickerLabel",
          [
            ("key", task.key().to_string().as_str()),
            ("title", task.details().title.as_str()),
            (
              "status",
              ctx
                .t(crate::tasks::organization::workflow_copy(task.organization().status()).1)
                .as_ref(),
            ),
          ],
        );
        let (choice, id) = (self.choice.clone(), task.id());
        menu_option(&label, chosen == Some(id), true, move || {
          choice.set(DependencyChoice(Some(id)));
        })
        .id(format!("task-blocking-option-{}", task.key()))
      })
      .collect();
    let current = tasks
      .tasks
      .iter()
      .filter(matches)
      .find(|task| Some(task.id()) == props.task)
      .map(|task| {
        Text::new(&ctx.t_args("tasks.blocked.currentTask", [("key", task.key().to_string().as_str())]))
          .variant(TypographyStyle::Caption)
      });
    let list = (!options.is_empty()).then(|| {
      Column::new()
        .id("task-blocking-picker")
        .width(Dimension::Pct(100.0))
        .max_width(Dimension::Px(RESULTS_WIDTH))
        .padding(SpacingSize::Xs)
        .background(PaletteColor::SurfaceRaised)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(RadiusSize::Md)
        .with_children(options)
    });
    Column::new()
      .spacing(SpacingSize::Md)
      .with_children(list)
      .with_children(current)
  }

  fn add_control(&self, ctx: &Ctx, props: &DependencyProps, tasks: &crate::tasks::data::ProjectTasks) -> Element {
    if !self.picking.get() {
      let picking = self.picking.clone();
      return ActionButton::new(ctx.t("tasks.blocked.addLink"))
        .icon(Icon::Plus)
        .in_form()
        .id("task-open-blocking-picker")
        .on_click(move || picking.set(true))
        .render(ctx);
    }
    let (choice, value, picking) = (self.choice.clone(), props.value.clone(), self.picking.clone());
    let cycle = self.creates_cycle(props, tasks);
    // Only a task the results show now can be added.
    let offered = self
      .choice
      .get()
      .0
      .is_some_and(|chosen| self.offered(props, tasks).iter().any(|task| task.id() == chosen));
    let (query, clear_choice) = (self.query.clone(), self.choice.clone());
    let add = ActionButton::new(ctx.t("tasks.blocked.addSelected"))
      .icon(Icon::Plus)
      .in_form()
      .id("task-add-blocking-link")
      .on_click(move || {
        if let Some(task) = choice.get().0 {
          value.update(|value| value.0 = with_link(&value.0, task));
          choice.set(DependencyChoice(None));
          picking.set(false);
        }
      })
      .enabled(offered && !cycle)
      .render(ctx);
    Column::new()
      .width(Dimension::Pct(100.0))
      .max_width(Dimension::Px(RESULTS_WIDTH))
      .spacing(SpacingSize::Md)
      .child(
        // The input writes what is typed into `query` itself; setting it here too would type each character twice.
        text_input(ctx, "blocking-task-search", query, false, TypographyStyle::Body)
          .placeholder(&ctx.t("tasks.blocked.search"))
          .on_input(move |_: lurq::app::events::TextInputEvent| clear_choice.set(DependencyChoice(None))),
      )
      .child(self.results(ctx, props, tasks))
      .child(add)
      .with_children(cycle.then(|| crate::ui::field::field_error(&ctx.t("tasks.blocked.cycle"))))
      .into()
  }
}

/// `organization` with `task` as one more blocking task (once).
fn with_link(organization: &TaskOrganization, task: TaskId) -> TaskOrganization {
  let mut links = organization.dependencies().to_vec();
  if !links.iter().any(|link| link.task == task) {
    links.push(BlockingTask {
      task,
      gate: DependencyGate::Merge,
    });
  }
  TaskOrganization::new(
    organization.status(),
    organization.priority(),
    organization.blocked_reason().map(str::to_owned),
    links,
    organization.tags().to_vec(),
  )
  .expect("adding a link preserves local organization invariants; the store validates cycles")
}

fn readiness_note(ctx: &Ctx, tasks: &crate::tasks::data::ProjectTasks, unfinished: &[TaskId], reason: &str) -> Element {
  let mut column = Column::new();
  if !unfinished.is_empty() {
    let keys = unfinished
      .iter()
      .filter_map(|id| tasks.tasks.iter().find(|task| task.id() == *id))
      .map(|task| task.key().to_string())
      .collect::<Vec<_>>()
      .join(&ctx.t("common.listSeparator"));
    column = column.child(crate::ui::field::wrapping_field_error(&ctx.t_args(
      "tasks.blocked.unfinished",
      [("tasks", keys.as_str()), ("reason", reason)],
    )));
  }
  column.into()
}

// ZLhiM free-text reason field.
const BLOCKED_REASON_HEIGHT: f32 = 96.0;

/// The Blocked checkbox; `enabled` is false in the locked form, where it only shows the value.
fn blocked_control(ctx: &Ctx, checked: bool, value: Signal<bool>, enabled: bool) -> Element {
  let content = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(lurq::layout::Alignment::Center)
    .child(crate::ui::icon::icon(
      if checked { Icon::SquareCheck } else { Icon::Square },
      crate::ui::icon::IconSize::Option,
      lurq::app::theme::PaletteColor::TextPrimary,
    ))
    .child(Text::new(&ctx.t("tasks.blocked.label")).variant(TypographyStyle::Body));
  let surface = crate::ui::pressable::surface().id("task-blocked-field").child(content);
  let button = if enabled {
    crate::ui::pressable::keyboard_stop(
      surface
        .cursor(lurq::node::CursorIcon::Pointer)
        .hovered_style(lurq::node::Style::new().background(lurq::app::theme::PaletteColor::AccentMuted))
        .on_click(move |_| value.update(|value| *value = !*value)),
      crate::ui::pressable::Ring::Outer,
    )
  } else {
    surface
  };
  Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(lurq::layout::Alignment::Center)
    .child(button)
    .child(Text::new(&ctx.t("tasks.blocked.helper")).variant(TypographyStyle::Caption))
    .into()
}
