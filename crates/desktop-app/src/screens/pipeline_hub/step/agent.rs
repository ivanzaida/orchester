//! *Agent* of an open step (`Agent` `fDeUM` in P2 `u2rYEH`): today's Executor menu, which lists the project's agent
//! profiles and then its tools, until the Agent picker replaces it. The box fills the row's value, so it measures the
//! room its row gives it. A choice changes the edit the panel holds.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Row,
  core::{ElementRef, Ref, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::{Step, StepKey};
use orchester_workspace::pipelines::{
  data::{Assignables, DraftProfileChoice},
  fields::Executor,
};

use crate::{
  screens::{
    pipeline_hub::edit::Edit,
    pipelines::executor_menu::{Entry, ExecutorField, Tone},
  },
  ui::icon::Icon,
};

/// The box's width until its row has been laid out: the old step page's.
const FIRST_WIDTH: f32 = 220.0;

/// What the menu lists for step `step`, and how its box is outlined.
pub(super) struct Choices {
  pub profiles: Vec<Entry>,
  pub tools: Vec<Entry>,
  pub tone: Tone,
  /// The profile the step runs when it is only a draft, which cannot run yet.
  pub draft: Option<DraftProfileChoice>,
}

/// The profiles and tools of `assignables` for `step`; `draft_label` words a draft profile and `unavailable` and
/// `tool_unavailable` say why one cannot be chosen.
pub(super) fn choices(
  assignables: &Assignables,
  step: &Step,
  (draft_label, unavailable, tool_unavailable): (impl Fn(&str) -> Arc<str>, Arc<str>, Arc<str>),
) -> Choices {
  let draft = match Executor::of(step) {
    Executor::Profile(id) => assignables
      .draft_profiles
      .iter()
      .find(|profile| profile.id == id)
      .cloned(),
    Executor::Tool(_) | Executor::Unset => None,
  };
  let mut profiles: Vec<Entry> = assignables
    .profiles
    .iter()
    .map(|profile| Entry {
      value: Executor::Profile(profile.id),
      label: profile.name.clone(),
      icon: Icon::Bot,
      unavailable: None,
    })
    .collect();
  if let Some(profile) = &draft {
    profiles.push(Entry {
      value: Executor::Profile(profile.id),
      label: draft_label(&profile.name),
      icon: Icon::Bot,
      unavailable: Some(unavailable),
    });
  }
  let tools = assignables
    .tools
    .iter()
    .map(|tool| Entry {
      value: Executor::Tool(tool.id),
      label: tool.name.clone(),
      icon: Icon::Wrench,
      unavailable: (!tool.runnable).then(|| tool_unavailable.clone()),
    })
    .collect();
  let tone = if Executor::of(step) == Executor::Unset {
    Tone::Invalid
  } else if draft.is_some() {
    Tone::Warning
  } else {
    Tone::Normal
  };
  Choices {
    profiles,
    tools,
    tone,
    draft,
  }
}

#[derive(Clone)]
pub(super) struct Props {
  pub edit: Edit,
  pub key: StepKey,
  /// What runs the step now, which the box starts with.
  pub value: Executor,
  pub profiles: Vec<Entry>,
  pub tools: Vec<Entry>,
  pub tone: Tone,
  pub name: Arc<str>,
  pub unset: Arc<str>,
  pub locked: bool,
  pub epoch: u64,
}

/// The props are equal when they say the same: the edit's handle is the panel's own.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (
      &self.key,
      self.value,
      &self.profiles,
      &self.tools,
      self.tone,
      &self.name,
      &self.unset,
      self.locked,
    ) == (
      &other.key,
      other.value,
      &other.profiles,
      &other.tools,
      other.tone,
      &other.name,
      &other.unset,
      other.locked,
    )
  }
}
impl DevtoolsInspectable for Props {}

/// The menu of `props`, mounted anew for each step and after each save or *Cancel*.
pub(super) fn field(ctx: &mut Ctx, props: Props) -> Element {
  let key = format!("pipeline-hub-step-agent-{}-{}", props.key, props.epoch);
  ctx.mount_keyed::<Agent>(&key, props)
}

struct Agent {
  executor: Signal<Executor>,
  /// The panel's edit as of the last draw, which a choice goes to.
  edit: Ref<Option<Edit>>,
  row: ElementRef,
  width: Signal<f32>,
}

impl Component for Agent {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let this = Self {
      executor: ctx.signal(props.value),
      edit: ctx.create_ref(Some(props.edit.clone())),
      row: ctx.element_ref(),
      width: ctx.signal(FIRST_WIDTH),
    };
    let (edit, key) = (this.edit.clone(), props.key);
    ctx.watch(&this.executor, move |chosen: &Executor| {
      if let Some(edit) = edit.get() {
        edit.change(|current| {
          current.update(&key, |step| chosen.apply(step));
        });
      }
    });
    this
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    self.edit.set(Some(props.edit.clone()));
    let width = self.width.clone();
    self.row.observe_rect(move |rect| {
      if rect.width > 0.0 && (width.get_untracked() - rect.width).abs() > 0.5 {
        width.set(rect.width);
      }
    });
    let field = ExecutorField {
      id: "pipeline-hub-step-agent".into(),
      name: props.name,
      value: self.executor.clone(),
      unset: props.unset,
      profiles: props.profiles,
      tools: props.tools,
      width: self.width.get(),
      tone: props.tone,
      locked: props.locked,
    };
    Row::new()
      .id("pipeline-hub-step-agent-room")
      .ref_element(self.row.clone())
      .width(Dimension::Pct(100.0))
      .child(field.render(ctx))
  }
}
