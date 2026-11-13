//! The open step's ⋯ (`More` `Xvbxz` in P2 `u2rYEH`, `eUqAD` in P2b `EGEos`): a 28 square icon button beside
//! *Duplicate* that opens the flow card's menu for the step (`Step menu` `kzSvl` in P4 `naN3G`) — the card's own rows,
//! rules, words and reasons ([`crate::screens::pipeline_hub::flow::StepMenu`]), not a copy of them. A copy or a step
//! added from it opens, as the head's *Duplicate* does; *Remove step* leaves no step to show, so the panel shows the
//! pipeline's flow, where a card's *Remove step* leaves the person too. Alt ↑ and Alt ↓ move the open step, as they
//! move a card.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Row,
  core::ElementRef,
  node::Element,
};
use orchester_core::{
  id::{ProjectId, ResourceId},
  resource::StepKey,
};
use orchester_workspace::pipelines::flow::{FlowRead, PipelineFlow, StepCard};

use crate::{
  navigation::Navigation,
  routes::{PipelineView, ProjectPage, Route},
  screens::pipeline_hub::{
    edit::Edit,
    flow::{OpensMade, StepMenu, Target},
  },
  ui::{icon::Icon, icon_button::IconButton},
};

#[derive(Clone)]
pub(super) struct Props {
  pub edit: Edit,
  /// The panel's read of what it shows, whose flow has the step's card when its steps have an order to draw.
  pub read: Arc<FlowRead>,
  pub key: StepKey,
  pub project: ProjectId,
  pub pipeline: ResourceId,
  /// A save is under way or a question about it waits: the ⋯ takes no press.
  pub locked: bool,
}

/// The props are equal when they say the same: the edit's handle is the panel's own.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (&self.read, &self.key, self.project, self.pipeline, self.locked)
      == (&other.read, &other.key, other.project, other.pipeline, other.locked)
  }
}
impl DevtoolsInspectable for Props {}

/// The ⋯ of `props`.
pub(super) fn more(ctx: &mut Ctx, props: Props) -> Element {
  ctx.mount_keyed::<More>("pipeline-hub-step-more-host", props)
}

struct More {
  menu: StepMenu,
  anchor: ElementRef,
}

impl Component for More {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      menu: StepMenu::new(ctx),
      anchor: ElementRef::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    self.menu.lock(props.locked);
    let flow = props.read.flow.as_ref().ok();
    let card = flow.and_then(|flow| card_of(flow, &props.key)).cloned();
    let open = self.menu.open().get();
    let name = card.as_ref().map_or("", |card| card.name.as_ref());
    let label = ctx.t_args("pipeline_hub.flow.move.more", [("name", name)]);
    let mut button = IconButton::new("pipeline-hub-step-more", Icon::Ellipsis, label).element(self.anchor.clone());
    if open {
      button = button.active();
    }
    // A step the flow cannot draw has no card, and so no menu: the ⋯ keeps its place, faded.
    if !props.locked && card.is_some() {
      let menu = self.menu.clone();
      button = button.on_click(move || menu.toggle());
    }
    // Alt ↑ and Alt ↓ reach every element: the head's moves the open step, as the flow's moves the card the keys are
    // on.
    let mut row = Row::new()
      .on_key_down(self.menu.keys(&props.edit, &props.key))
      .child(button.render(ctx));
    if let (Some(card), Some(flow), true) = (card.as_ref(), flow, open) {
      row = row.child(self.popup(ctx, &props, (flow, card)));
    }
    row
  }
}

impl More {
  /// The card's menu for the open step, under the ⋯; a step it makes opens.
  fn popup(&self, ctx: &mut Ctx, props: &Props, (flow, card): (&PipelineFlow, &StepCard)) -> Element {
    let (navigation, project, pipeline) = (Navigation::of(ctx), props.project, props.pipeline);
    let opens: OpensMade = Arc::new(move |made| {
      navigation.go(Route::Project(
        project,
        ProjectPage::Pipeline(pipeline, PipelineView::Step(made)),
      ));
    });
    let target = Target {
      edit: &props.edit,
      flow,
      card,
    };
    self.menu.popup(ctx, &target, (self.anchor.clone(), opens)).into()
  }
}

/// The card of step `key` in `flow`.
fn card_of<'a>(flow: &'a PipelineFlow, key: &StepKey) -> Option<&'a StepCard> {
  flow
    .stages
    .iter()
    .flat_map(|stage| stage.cards.iter())
    .find(|card| card.key == *key)
}
