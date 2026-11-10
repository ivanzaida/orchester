//! *Agent* of an open step (`Agent` `fDeUM` in P2 `u2rYEH`, open in P10 `HjgZ8`): one field in place of the Executor
//! and model boxes. It shows the profile's mark, its name and what it runs on, and opens the Agent picker under it,
//! from its start. A step is run by an agent profile or a tool, never by a model: the profile says what it runs on. A
//! step that overrides the model as it was stored keeps it, and the field and the picker's chosen row say so (*model:
//! <id>*). Choosing changes the edit the panel holds and nothing else.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, HitTestBehavior, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, resource::StepKey};
use orchester_workspace::pipelines::fields::Executor;

use super::choices::Flag;
use crate::{
  navigation::Navigation,
  routes::{AgentTab, ProjectPage, Route},
  screens::pipeline_hub::edit::Edit,
  theme::{AgentPickerText, ControlHeight, DISABLED_OPACITY, ExtraSpacing, IconSize},
  ui::{
    agent_picker::{self, Actions, AgentPicker, Item},
    icon::{Icon, icon},
    popover::{PopoverSide, popover},
    pressable::{Focus, pressable},
  },
};

#[derive(Clone)]
pub(super) struct Props {
  pub edit: Edit,
  pub project: ProjectId,
  pub key: StepKey,
  /// What runs the step now.
  pub value: Executor,
  pub choices: Arc<[Item]>,
  pub executors: Arc<[Executor]>,
  pub flag: Option<Flag>,
  pub name: Arc<str>,
  pub unset: Arc<str>,
  pub locked: bool,
  pub epoch: u64,
}

/// The props are equal when they say the same: the edit's handle is the panel's own.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (
      self.project,
      &self.key,
      self.value,
      &self.choices,
      &self.executors,
      self.flag,
      &self.name,
      &self.unset,
      self.locked,
    ) == (
      other.project,
      &other.key,
      other.value,
      &other.choices,
      &other.executors,
      other.flag,
      &other.name,
      &other.unset,
      other.locked,
    )
  }
}
impl DevtoolsInspectable for Props {}

/// The field of `props`, mounted anew for each step and after each save or *Cancel*.
pub(super) fn field(ctx: &mut Ctx, props: Props) -> Element {
  let key = format!("pipeline-hub-step-agent-{}-{}", props.key, props.epoch);
  ctx.mount_keyed::<Agent>(&key, props)
}

struct Agent {
  anchor: ElementRef,
  open: Signal<bool>,
}

impl Component for Agent {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      anchor: ElementRef::new(),
      open: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    if props.locked && self.open.get_untracked() {
      self.open.set(false);
    }
    let open = self.open.get();
    let trigger = self.trigger(ctx, &props, open);
    let row = Row::new().width(Dimension::Pct(100.0)).child(trigger);
    if !open {
      return row;
    }
    row.child(popover(
      self.anchor.clone(),
      self.open.clone(),
      PopoverSide::Below,
      self.picker(ctx, &props),
    ))
  }
}

impl Agent {
  /// The picker over the step's choices: choosing one changes the step's executor and closes it.
  fn picker(&self, ctx: &mut Ctx, props: &Props) -> Element {
    let chosen = props
      .executors
      .iter()
      .position(|executor| *executor == props.value)
      .map(|at| props.choices[at].key.clone());
    let (edit, step, executors, items) = (
      props.edit.clone(),
      props.key.clone(),
      props.executors.clone(),
      props.choices.clone(),
    );
    let open = self.open.clone();
    let picked = open.clone();
    let (navigation, project) = (Navigation::of(ctx), props.project);
    let (new, agents) = (navigation.clone(), navigation);
    let (close_new, close_agents) = (open.clone(), open.clone());
    AgentPicker::new(
      "pipeline-hub-step-agent-picker",
      (props.choices.to_vec(), chosen),
      Actions {
        on_pick: Box::new(move |key| {
          picked.set(false);
          if let Some(at) = items.iter().position(|item| item.key == key) {
            let executor = executors[at];
            edit.change(|current| {
              current.update(&step, |step| executor.apply(step));
            });
          }
        }),
        on_close: Box::new(move || open.set(false)),
        on_new: Box::new(move || {
          close_new.set(false);
          new.go(Route::Project(project, ProjectPage::NewAgent(AgentTab::default())));
        }),
        on_agents: Box::new(move || {
          close_agents.set(false);
          agents.go(Route::Project(project, ProjectPage::Agents));
        }),
      },
    )
    .element(ctx)
  }

  /// The box (`Input` `rMu3f`): 28 high on `surface_input`, the chosen agent's mark, its name and what it runs on, and
  /// the chevron; the `accent` stroke while open, `danger` with no agent chosen and `warning` for one that is not ready
  /// or gone; what is wrong with it is the field's own second line, in place of what it runs on, so fixing it moves
  /// nothing under the field.
  fn trigger(&self, ctx: &Ctx, props: &Props, open: bool) -> Element {
    let current = props
      .executors
      .iter()
      .position(|executor| *executor == props.value)
      .map(|at| &props.choices[at]);
    let stroke = match (props.flag, open) {
      (Some(Flag::Unset), _) => PaletteColor::Danger,
      (Some(Flag::NotReady), _) => PaletteColor::Warning,
      (None, true) => PaletteColor::Accent,
      (None, false) => PaletteColor::Border,
    };
    let toggle = self.open.clone();
    let mut row = Focus::Keyboard
      .apply(pressable(move || toggle.update(|open| *open = !*open)))
      .id("pipeline-hub-step-agent")
      .ref_element(self.anchor.clone())
      .describe("a11y_name", props.name.clone())
      .describe("state", if open { "open" } else { "closed" })
      .describe("locked", props.locked.to_string())
      .hit_test(if props.locked {
        HitTestBehavior::None
      } else {
        HitTestBehavior::Auto
      })
      .opacity(if props.locked { DISABLED_OPACITY } else { 1.0 })
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ControlHeight::Md.value()))
      .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Md)
      .background(PaletteColor::SurfaceInput)
      .border_inside(BorderSize::Sm, stroke.clone())
      .hovered_style(Style::new().border_inside(
        BorderSize::Sm,
        if props.flag.is_none() && !open {
          PaletteColor::BorderStrong
        } else {
          stroke
        },
      ));
    row = match props.flag {
      Some(Flag::Unset) => row.class("unset"),
      Some(Flag::NotReady) => row.class("not-ready"),
      None => row,
    };
    row = match current {
      Some(item) => row
        .child(agent_picker::mark(ctx, item.mark))
        .child(
          Text::new(&item.name)
            .id("pipeline-hub-step-agent-name")
            .variant(AgentPickerText::FieldName.role())
            .nowrap(),
        )
        .child(
          Row::new()
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .with_children(agent_picker::second_line(
              "pipeline-hub-step-agent-runs".to_owned(),
              item,
              AgentPickerText::FieldRuns.role(),
            )),
        ),
      None => row.child(
        Text::new(&props.unset)
          .id("pipeline-hub-step-agent-name")
          .variant(AgentPickerText::FieldRuns.role())
          .flex(1.0)
          .nowrap(),
      ),
    };
    row
      .child(icon(
        if open { Icon::ChevronUp } else { Icon::ChevronDown },
        IconSize::Md,
        PaletteColor::TextMuted,
      ))
      .into()
  }
}
