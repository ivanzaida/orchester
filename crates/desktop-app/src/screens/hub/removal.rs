//! Remove, asked in place under an opened item's head (`06c`): a profile, instruction, skill or tool no one uses is
//! removed after a plain question; one in use lists who uses it, and *Remove and reset* removes it and clears it from
//! exactly the steps listed (`orchester_workspace::removal`). When the steps that run it are others by then, the
//! removal changes nothing and the question asks again with them.
//!
//! Removing changes what agents are told (an instruction's or skill's text leaves their prompts) or which steps run
//! what (a profile's or tool's steps are reset), so lurq's synthetic input (an MCP client driving the window) removes
//! only under *Let automation change agent permissions*, as Publish does; otherwise the question says it is a person's.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
  node::Element,
};
use orchester_core::{
  id::ResourceId,
  resource::{Removal, RemoveMode, ResourceKind, StepUser, StepVersion},
};
use orchester_workspace::{
  background,
  messages::Message,
  prompt_changes,
  removal::{self, Reached, Users},
};

use super::question::{Question, Weight};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  project::ShownProject,
  services::Services,
  ui::{icon::Icon, message::message_text},
};

type Handler = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone)]
pub(crate) struct RemoveProps {
  pub project: ShownProject,
  pub resource: ResourceId,
  pub kind: ResourceKind,
  pub name: Arc<str>,
  /// Keep it: the question closes.
  pub on_keep: Handler,
  /// It was removed; the changes are recorded already.
  pub on_removed: Handler,
}

impl PartialEq for RemoveProps {
  fn eq(&self, other: &Self) -> bool {
    (self.resource, self.kind, &self.name, &self.project) == (other.resource, other.kind, &other.name, &other.project)
  }
}

impl DevtoolsInspectable for RemoveProps {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Removed(Removal);

impl DevtoolsInspectable for Removed {}

/// The pipeline steps the removal itself found using it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Found(Vec<StepUser>);

impl DevtoolsInspectable for Found {}

/// The question, mounted while it asks.
pub(crate) fn question(ctx: &mut Ctx, props: RemoveProps) -> Element {
  ctx.mount_keyed::<RemoveQuestion>(&format!("hub-remove-{}", props.resource), props)
}

pub(crate) struct RemoveQuestion {
  work: FutureAction<RemoveMode, Removed, Message>,
  /// Who the removal found using it, when that is not who the question listed.
  found: Signal<Option<Found>>,
  gate: Option<InputGate>,
  /// An MCP client's input pressed the confirm without the person's consent; nothing was removed.
  refused: Signal<bool>,
}

impl Component for RemoveQuestion {
  type Props = RemoveProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<RemoveProps>().clone();
    let root = props.project.open.repo_root.clone();
    let work = ctx.future_action(move |mode: RemoveMode| {
      let root = root.clone();
      async move { background::run(move || removal::remove_in(&root, props.resource, mode).map(Removed)).await? }
    });
    let found = ctx.signal(None);
    let (changes, project, kind) = (Services::of(ctx).changes, props.project.open.project.id(), props.kind);
    let (seen, removed) = (found.clone(), props.on_removed.clone());
    ctx.watch(&work.state(), move |state| {
      match state.data.clone().filter(|_| state.is_fulfilled()) {
        Some(Removed(Removal::Removed)) => {
          for change in removal::changed_by_removal(kind) {
            changes.record(project, *change);
          }
          removed();
        }
        Some(Removed(Removal::InUse(users))) => seen.set(Some(Found(users))),
        None => {}
      }
    });
    Self {
      work,
      found,
      gate: ctx.use_context::<InputGate>(),
      refused: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RemoveProps>().clone();
    let root = props.project.open.repo_root.clone();
    let (resource, kind) = (props.resource, props.kind);
    let read = ctx
      .future((root, resource), move |(root, resource)| async move {
        background::run(move || removal::users_in(&root, resource, kind)).await?
      })
      .state()
      .get();
    // Until who uses it is known (read, or found by a removal), nothing is offered: a failed read shows its failure
    // instead of offering it as unused.
    let known = self.found.get().is_some() || read.data.is_some();
    let users = match (self.found.get(), read.data.clone()) {
      (Some(Found(found)), _) => Users::Steps(found),
      (None, Some(users)) => users,
      (None, None) => Users::Steps(Vec::new()),
    };
    let state = self.work.state().get();
    let failure = if self.refused.get() {
      Some(message_text(ctx, &prompt_changes::removal_person_only()))
    } else {
      state
        .error
        .clone()
        .filter(|_| state.is_rejected())
        .or_else(|| read.error.clone().filter(|_| read.is_rejected()))
        .map(|error| message_text(ctx, &error))
    };
    let busy = state.is_pending() || !known;
    // Reset goes on only with the steps listed here; any other use makes the removal ask again.
    let mode = match &users {
      Users::Steps(steps) if !steps.is_empty() => RemoveMode::Reset(steps.clone()),
      Users::Steps(_) | Users::Reach(_) => RemoveMode::Unused,
    };
    let title = ctx.t_args("hub.remove.title", [("name", &*props.name)]);
    let (sentence, confirm) = words(ctx, kind, &users);
    let mut question = Question::new(
      "hub-remove",
      Weight::Loses,
      (title, sentence),
      ((Icon::Trash2, confirm), ctx.t("hub.remove.keep")),
    );
    for (glyph, line) in lines(ctx, &users) {
      question = question.line(glyph, line);
    }
    let (work, keep) = (self.work.clone(), props.on_keep.clone());
    let (gate, refused) = (self.gate.clone(), self.refused.clone());
    Element::from(
      question
        .failure(failure)
        .on_confirm(busy, move || {
          let may = input_may_decide(gate.as_ref(), Guarded::PermissionWidening);
          refused.set(!may);
          if may {
            work.run(mode.clone());
          } else {
            tracing::warn!("synthetic input pressed Remove; not taken");
          }
        })
        .on_keep(move || keep()),
    )
  }
}

/// The sentence and the action's label for `users` of a resource of `kind`.
fn words(ctx: &Ctx, kind: ResourceKind, users: &Users) -> (Arc<str>, Arc<str>) {
  if users.is_empty() {
    let key = match kind {
      ResourceKind::Instruction => "hub.remove.unusedInstruction",
      ResourceKind::Skill => "hub.remove.unusedSkill",
      ResourceKind::AgentProfile
      | ResourceKind::Tool
      | ResourceKind::Pipeline
      | ResourceKind::TestSuite
      | ResourceKind::Service
      | ResourceKind::Stack => "hub.remove.unused",
    };
    return (ctx.t(key), ctx.t("hub.remove.confirm"));
  }
  let key = match (kind, users) {
    (ResourceKind::Tool, _) => "hub.remove.inUseTool",
    (_, Users::Reach(reached)) if reached.len() == 1 => "hub.remove.inUseSkillOne",
    (_, Users::Reach(_)) => "hub.remove.inUseSkillOther",
    _ => "hub.remove.inUseProfile",
  };
  let named = match users {
    Users::Reach(reached) => match reached.as_slice() {
      [Reached::Profile { name, .. }] => name.to_string(),
      _ => String::new(),
    },
    Users::Steps(_) => String::new(),
  };
  (ctx.t_args(key, [("name", named)]), ctx.t("hub.remove.reset"))
}

/// One line for each user, with its glyph.
fn lines(ctx: &Ctx, users: &Users) -> Vec<(Icon, Arc<str>)> {
  match users {
    Users::Steps(steps) => steps
      .iter()
      .map(|user| {
        let number = format!("{:02}", user.number);
        let (key, version) = match user.version {
          StepVersion::Published(version) => ("hub.remove.stepPublished", version.get()),
          StepVersion::Draft(version) => ("hub.remove.stepDraft", version),
        };
        let text = ctx.t_args(
          key,
          [
            ("pipeline", user.pipeline_name.as_str().to_owned()),
            ("version", version.to_string()),
            ("number", number),
            ("step", user.step_name.as_str().to_owned()),
          ],
        );
        (Icon::Workflow, text)
      })
      .collect(),
    Users::Reach(reached) => reached
      .iter()
      .map(|reached| match reached {
        Reached::Profile { name, by_role: true } => {
          (Icon::Bot, ctx.t_args("hub.remove.reachRole", [("name", &**name)]))
        }
        Reached::Profile { name, by_role: false } => {
          (Icon::Bot, ctx.t_args("hub.remove.reachProject", [("name", &**name)]))
        }
        Reached::Pipeline(name) => (
          Icon::Workflow,
          ctx.t_args("hub.remove.reachPipeline", [("name", &**name)]),
        ),
        Reached::Step { pipeline, step } => (
          Icon::CornerDownRight,
          ctx.t_args("hub.remove.reachStep", [("pipeline", &**pipeline), ("step", &**step)]),
        ),
      })
      .collect(),
  }
}
