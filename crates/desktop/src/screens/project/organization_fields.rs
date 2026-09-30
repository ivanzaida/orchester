use std::path::PathBuf;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::TaskId,
  task::organization::{BlockingTask, DependencyGate, OrganizationError, Priority, TaskOrganization, WorkflowStatus},
};

use super::tag_picker::{TagPicker, TagPickerProps};
use crate::{
  tasks::organization::{
    OrganizationValue, PickerAppearance, PriorityChoice, WorkflowChoice, priority_select, workflow_select,
  },
  ui::{
    button::ActionButton,
    field::{FieldNote, field, text_input},
    icon::Icon,
  },
};

#[derive(Clone)]
pub struct OrganizationDraft {
  pub status: Signal<WorkflowChoice>,
  pub priority: Signal<PriorityChoice>,
  pub blocked: Signal<bool>,
  pub reason: Signal<String>,
  pub value: Signal<OrganizationValue>,
  pub ready: Signal<bool>,
}

impl OrganizationDraft {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      status: ctx.signal(WorkflowChoice(WorkflowStatus::Backlog)),
      priority: ctx.signal(PriorityChoice(Priority::None)),
      blocked: ctx.signal(false),
      reason: ctx.signal(String::new()),
      value: ctx.signal(OrganizationValue::default()),
      ready: ctx.signal(true),
    }
  }
  pub fn seed(&self, value: &TaskOrganization) {
    self.status.set(WorkflowChoice(value.status()));
    self.priority.set(PriorityChoice(value.priority()));
    self.blocked.set(value.blocked_reason().is_some());
    self.reason.set(value.blocked_reason().unwrap_or_default().to_owned());
    self.value.set(OrganizationValue(value.clone()));
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
}
impl PartialEq for OrganizationFieldsProps {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root && self.task == other.task
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
      .child(organization_pickers(ctx, draft))
      .child(ctx.mount_keyed::<TagPicker>(
        "form-tags",
        TagPickerProps {
          root: props.root.clone(),
          selected: draft.value.clone(),
          enabled: true,
        },
      ))
      .child(
        ActionButton::new(ctx.t("tasks.blocked.label"))
          .icon(if blocked { Icon::SquareCheck } else { Icon::Square })
          .id("task-blocked-field")
          .in_form()
          .on_click(move || flip.update(|value| *value = !*value))
          .render(ctx),
      );
    if blocked {
      let control: Element = text_input(
        ctx,
        "task-blocked-reason",
        reason.clone(),
        draft.reason.get().trim().is_empty(),
        TypographyStyle::Body,
      )
      .on_input(move |event: lurq::app::events::TextInputEvent| reason.set(event.new_value))
      .into();
      content = content.child(field(&ctx.t("tasks.blocked.reason"), control, note));
    }
    if blocked || !draft.value.get().0.dependencies().is_empty() {
      content = content.child(ctx.mount_keyed::<DependencyFields>(
        "blocking-links",
        DependencyProps {
          root: props.root,
          task: props.task,
          value: draft.value.clone(),
          ready: draft.ready.clone(),
          reason: draft.reason.clone(),
          blocked: draft.blocked.clone(),
        },
      ));
    } else {
      draft.ready.set(true);
    }
    content
  }
}

fn organization_pickers(ctx: &Ctx, draft: &OrganizationDraft) -> Element {
  Row::new()
    .spacing(SpacingSize::Lg)
    .child(Column::new().flex(1.0).child(field(
      &ctx.t("tasks.workflow.label"),
      workflow_select(
        ctx,
        draft.status.clone(),
        "task-workflow-field",
        true,
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
        true,
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
  value: Signal<OrganizationValue>,
  ready: Signal<bool>,
  reason: Signal<String>,
  blocked: Signal<bool>,
}
impl PartialEq for DependencyProps {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root && self.task == other.task
  }
}
impl lurq::app::component::DevtoolsInspectable for DependencyProps {}
struct DependencyFields {
  choice: Signal<DependencyChoice>,
  query: Signal<String>,
}
impl Component for DependencyFields {
  type Props = DependencyProps;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      choice: ctx.signal(DependencyChoice(None)),
      query: ctx.signal(String::new()),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DependencyProps>().clone();
    let shown = ctx
      .use_context::<super::pages::Shown>()
      .expect("shell provides project data");
    let load = ctx.future((props.root.clone(), shown.revision), |(root, _)| async move {
      crate::background::run(move || crate::tasks::data::ProjectTasks::load(&root))
        .await?
        .map_err(|error| crate::messages::store_error(&error))
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
    for link in organization.dependencies() {
      let id = link.task;
      let label = tasks
        .tasks
        .iter()
        .find(|task| task.id() == id)
        .map(|task| task.key().to_string())
        .unwrap_or_else(|| id.to_string());
      let value = props.value.clone();
      column = column.child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .child(Text::new(&label).variant(TypographyStyle::Caption))
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
              .render(ctx),
          ),
      );
    }
    column.child(self.add_control(ctx, &props, &tasks))
  }
}

impl DependencyFields {
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

  fn selector(&self, ctx: &Ctx, props: &DependencyProps, tasks: &crate::tasks::data::ProjectTasks) -> Element {
    let query = self.query.get().trim().to_lowercase();
    let options = std::iter::once((DependencyChoice(None), ctx.t("tasks.blocked.addLink"))).chain(
      tasks
        .tasks
        .iter()
        .filter(|task| {
          Some(task.id()) != props.task
            && (task.details().title.as_str().to_lowercase().contains(&query)
              || task.key().to_string().to_lowercase().contains(&query))
        })
        .map(|task| {
          (
            DependencyChoice(Some(task.id())),
            ctx.t_args(
              "tasks.blocked.linkLabel",
              [
                ("key", task.key().to_string().as_str()),
                ("title", task.details().title.as_str()),
              ],
            ),
          )
        }),
    );
    let select: Element = Element::from(
      lurq::components::Select::new(self.choice.clone())
        .focusable(tasks.tasks.iter().any(|task| Some(task.id()) != props.task))
        .options(options)
        .height(Dimension::Px(crate::tasks::organization::CONTROL_HEIGHT))
        .style(crate::ui::field::select_style(
          ctx,
          crate::tasks::organization::CONTROL_HEIGHT,
        )),
    )
    .id("task-blocking-picker");
    select
  }

  fn add_control(&self, ctx: &Ctx, props: &DependencyProps, tasks: &crate::tasks::data::ProjectTasks) -> Element {
    let select = self.selector(ctx, props, tasks);
    let choice = self.choice.clone();
    let value = props.value.clone();
    let cycle = self.creates_cycle(props, tasks);
    let query = self.query.clone();
    let clear_choice = self.choice.clone();
    Column::new()
      .spacing(SpacingSize::Sm)
      .child(
        text_input(ctx, "blocking-task-search", query.clone(), false, TypographyStyle::Body)
          .placeholder(&ctx.t("tasks.blocked.search"))
          .on_input(move |event: lurq::app::events::TextInputEvent| {
            query.set(event.new_value);
            clear_choice.set(DependencyChoice(None));
          }),
      )
      .child(select)
      .with_children(cycle.then(|| crate::ui::field::field_error(&ctx.t("tasks.blocked.cycle"))))
      .child(
        ActionButton::new(ctx.t("tasks.blocked.addLink"))
          .in_form()
          .id("task-add-blocking-link")
          .on_click(move || {
            if let Some(task) = choice.get().0 {
              value.update(|value| {
                let old = &value.0;
                let mut links = old.dependencies().to_vec();
                if !links.iter().any(|link| link.task == task) {
                  links.push(BlockingTask {
                    task,
                    gate: DependencyGate::Merge,
                  });
                }
                value.0 = TaskOrganization::new(
                  old.status(),
                  old.priority(),
                  old.blocked_reason().map(str::to_owned),
                  links,
                  old.tags().to_vec(),
                )
                .expect("adding a link preserves local organization invariants; the store validates cycles");
              });
              choice.set(DependencyChoice(None));
            }
          })
          .enabled(self.choice.get().0.is_some() && !cycle)
          .render(ctx),
      )
      .into()
  }
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
    column = column.child(crate::ui::field::field_error(&ctx.t_args(
      "tasks.blocked.unfinished",
      [("tasks", keys.as_str()), ("reason", reason)],
    )));
  }
  column.into()
}
