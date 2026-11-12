//! *Conflicts* of the Deliver step (`Conflicts` `WEPpe` in P15 `nPaMj`): *Resolve conflicts with*, the agent profile
//! the Integrate step works with when syncing with the base branch conflicts. It offers the profiles of the pipeline's
//! steps that can change files; without a choice, the step that implements the change does, and its profile shows. The
//! box is the step's Agent field's ([`super::super::agent::field_box`]) and opens the Agent picker over those profiles.
//! With no step that can change files, conflicts go to the person and the row says so, with no box; a pipeline whose
//! steps are not valid says to fix them first.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{ProjectId, ResourceId},
  resource::{DeliverStep, Pipeline},
};
use orchester_workspace::pipelines::{
  data::Assignables,
  deliver::{integrating_step, integrator_profiles},
};

use super::{
  super::{
    agent::{FieldBox, field_box},
    choices::profile_item,
  },
  Open, change,
};
use crate::{
  navigation::Navigation,
  routes::{AgentTab, ProjectPage, Route},
  screens::{pipeline_hub::edit::Edit, team_hub::rows},
  theme::HubText,
  ui::{
    agent_picker::{Actions, AgentPicker, Group, Item, Mark},
    icon::Icon,
    popover::{PopoverSide, popover},
  },
};

/// The box's width (`Select` `r5wErx`).
const BOX_WIDTH: f32 = 340.0;
const ROW: &str = "pipeline-hub-deliver-integrator-row";
const HINT: &str = "pipeline-hub-deliver-integrator-hint";

pub(super) fn group(ctx: &mut Ctx, open: &Open<'_>, deliver: &DeliverStep) -> Element {
  let label = ctx.t("pipeline_hub.deliver.resolveWith");
  let value = match open.current.validate() {
    Err(_) => instead(&ctx.t("pipeline_hub.deliver.integratorUnknown")),
    Ok(pipeline) => {
      let profiles = integrator_profiles(&pipeline);
      if profiles.is_empty() {
        instead(&ctx.t("pipeline_hub.deliver.noIntegrator"))
      } else {
        let props = props(ctx, open, (&pipeline, deliver), &profiles);
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(SpacingSize::Sm)
          .child(
            Row::new()
              .width(Dimension::Px(BOX_WIDTH))
              .child(ctx.mount_keyed::<Integrator>(&format!("pipeline-hub-deliver-integrator-{}", open.epoch), props)),
          )
          .child(hint(&ctx.t("pipeline_hub.deliver.integratorHint")))
          .into()
      }
    }
  };
  rows::section(
    "pipeline-hub-deliver-conflicts",
    &ctx.t("pipeline_hub.deliver.conflicts"),
    Vec::new(),
    vec![rows::row(ROW, &label, None, value)],
  )
  .into()
}

/// What the row says in place of the box.
fn instead(words: &str) -> Element {
  hint(words).flex(1.0).min_width(Dimension::Px(0.0)).into()
}

/// What the row says under the box, in `text_muted`.
fn hint(words: &str) -> Text {
  Text::new(words)
    .id(HINT)
    .variant(HubText::Status.role())
    .color(PaletteColor::TextMuted)
    .width(Dimension::Pct(100.0))
}

/// The box's props: the profiles `profiles` of `pipeline`, and which one the Integrate step works with now — the one
/// `deliver` names, else the one of the step that implements the change.
fn props(ctx: &Ctx, open: &Open<'_>, (pipeline, deliver): (&Pipeline, &DeliverStep), profiles: &[ResourceId]) -> Props {
  let mut ids = profiles.to_vec();
  if let Some(chosen) = deliver.integrator().filter(|chosen| !ids.contains(chosen)) {
    ids.push(chosen);
  }
  let items = ids
    .iter()
    .map(|id| item(ctx, open.assignables, *id))
    .collect::<Vec<_>>();
  let now = deliver
    .integrator()
    .or_else(|| integrating_step(pipeline, deliver, |_| true).and_then(|step| step.kind.executor()));
  Props {
    edit: open.edit.clone(),
    project: open.project.open.project.id(),
    items: items.into(),
    ids: ids.into(),
    now,
    name: ctx.t("pipeline_hub.deliver.resolveWith"),
    unset: ctx.t("pipeline_hub.deliver.chooseProfile"),
    locked: open.locked,
  }
}

/// A profile as the Agent picker lists it; one the project does not offer as published (a draft, or gone) by the words
/// that say so.
fn item(ctx: &Ctx, assignables: &Assignables, id: ResourceId) -> Item {
  if let Some(profile) = assignables.profile(id) {
    return profile_item(ctx, assignables, profile, None);
  }
  Item {
    key: id.to_string().into(),
    group: Group::Profiles,
    mark: Mark::Icon(Icon::Bot),
    name: ctx.t("pipeline_hub.deliver.unknownProfile"),
    runs_on: Arc::from(""),
    model: None,
    suffix: None,
    warning: None,
  }
}

#[derive(Clone)]
struct Props {
  edit: Edit,
  project: ProjectId,
  items: Arc<[Item]>,
  /// The profile of each item, in their order.
  ids: Arc<[ResourceId]>,
  /// The profile the Integrate step works with now.
  now: Option<ResourceId>,
  name: Arc<str>,
  unset: Arc<str>,
  locked: bool,
}

/// The props are equal when they say the same: the edit's handle is the panel's own.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (
      self.project,
      &self.items,
      &self.ids,
      self.now,
      &self.name,
      &self.unset,
      self.locked,
    ) == (
      other.project,
      &other.items,
      &other.ids,
      other.now,
      &other.name,
      &other.unset,
      other.locked,
    )
  }
}
impl DevtoolsInspectable for Props {}

struct Integrator {
  anchor: ElementRef,
  open: Signal<bool>,
}

impl Component for Integrator {
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
    let current = props
      .now
      .and_then(|now| props.ids.iter().position(|id| *id == now))
      .map(|at| &props.items[at]);
    let field = field_box(
      ctx,
      &FieldBox {
        id: "pipeline-hub-deliver-integrator",
        anchor: &self.anchor,
        open: (&self.open, open),
        current,
        unset: &props.unset,
        flag: None,
        name: props.name.clone(),
        locked: props.locked,
      },
    );
    let mut row = Row::new().width(Dimension::Pct(100.0)).child(field);
    if open {
      row = row.child(popover(
        self.anchor.clone(),
        self.open.clone(),
        PopoverSide::Below,
        self.picker(ctx, &props),
      ));
    }
    row
  }
}

impl Integrator {
  /// The Agent picker over the profiles: choosing one makes it the Deliver step's integrator and closes it.
  fn picker(&self, ctx: &mut Ctx, props: &Props) -> Element {
    let chosen = props.now.map(|now| Arc::<str>::from(now.to_string()));
    let (edit, ids, items) = (props.edit.clone(), props.ids.clone(), props.items.clone());
    let (picked, open) = (self.open.clone(), self.open.clone());
    let (close_new, close_agents) = (self.open.clone(), self.open.clone());
    let (navigation, project) = (Navigation::of(ctx), props.project);
    let (new, agents) = (navigation.clone(), navigation);
    AgentPicker::new(
      "pipeline-hub-deliver-integrator-picker",
      (props.items.to_vec(), chosen),
      Actions {
        on_pick: Box::new(move |key| {
          picked.set(false);
          if let Some(at) = items.iter().position(|item| item.key == key) {
            let profile = ids[at];
            change(&edit, |deliver| deliver.with_integrator(Some(profile)));
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
}
