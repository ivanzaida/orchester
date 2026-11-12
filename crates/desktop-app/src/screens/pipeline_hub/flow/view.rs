//! The flow as a component: it keeps what editing the order needs ([`super::state`]) and draws the rail with a card per
//! step ([`super::card`]). A card the pointer or the keys are on shows its handle and its ⋯ ([`super::drag`],
//! [`super::menu`]); Alt ↑ and Alt ↓ move the card the keys are on, Esc sets a lifted card down. Every change goes
//! through the edit the panel holds, so the flow draws the edit at once and Save keeps it.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::{KeyboardEvent, MouseEvent},
    theme::SpacingSize,
  },
  components::{Column, Row, Stack, Text},
  core::Signal,
  layout::{Alignment, StackAlignment},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ResourceId,
  resource::{PipelineEdit, StepKey},
};
use orchester_workspace::pipelines::flow::{FlowRead, PipelineFlow, Stage, StepCard};

use super::{
  Opens, add_deliver,
  card::{self, Look, Opening, Place},
  drag::{self, Lifted, LiftedProps},
  fix::Fix,
  landing::{self, Landing, Words},
  menu::{self, Menu},
  parts,
  rail::{self, Node},
  reorder::{Cards, Drive, keyed_move},
  state::{Drag, Drop, Held, Refused, Reorder},
  words,
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::PipelineView,
  screens::pipeline_hub::{edit::Edit, steps},
  theme::HubText,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    pressable::Focus,
  },
};

#[derive(Clone)]
pub(super) struct Props {
  pub project: ShownProject,
  pub pipeline: ResourceId,
  /// How many steps the pipeline has, for the head's count.
  pub count: usize,
  pub read: Arc<FlowRead>,
  /// The edit as the panel shows it, which the moves are tried on.
  pub current: PipelineEdit,
  pub edit: Edit,
  /// A save is under way or *Allow these changes?* is open: the flow offers no change, so none is lost when the save
  /// ends and what *Allow* publishes is what is on screen.
  pub locked: bool,
  /// The panel's: set by a card whose profile or tool is gone, so the step it opens opens its Agent picker.
  pub pick_agent: Signal<bool>,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (
      &self.project,
      self.pipeline,
      self.count,
      &self.read,
      &self.current,
      self.locked,
    ) == (
      &other.project,
      other.pipeline,
      other.count,
      &other.read,
      &other.current,
      other.locked,
    )
  }
}
impl DevtoolsInspectable for Props {}

pub(super) struct FlowView {
  reorder: Reorder,
}

impl Component for FlowView {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      reorder: Reorder::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let navigation = Navigation::of(ctx);
    let reorder = &self.reorder;
    reorder.clear_layout();
    reorder.drawn_from(&props.current);
    *reorder.locked.lock() = props.locked;
    if props.locked {
      release(reorder);
    }
    let drive = Drive {
      reorder: reorder.clone(),
      edit: props.edit.clone(),
      current: Arc::new(props.current.clone()),
    };
    let opens = Opens {
      navigation: &navigation,
      project: &props.project,
      pipeline: props.pipeline,
      read_only: false,
      pick_agent: Some(props.pick_agent.clone()),
    };
    let action = props
      .read
      .flow
      .as_ref()
      .ok()
      .map(|flow| head_action(ctx, &drive, flow, props.locked));
    let rows = self.rows(ctx, (&props, &drive, &opens));
    let list = Column::new()
      .id("pipeline-hub-flow")
      .width(Dimension::Pct(100.0))
      .on_key_down(keys(&drive))
      .with_children(rows);
    let floating = reorder.drag.get().filter(|_| !props.locked).map(|drag| {
      ctx.mount_keyed::<Lifted>(
        "pipeline-hub-flow-lifted",
        LiftedProps {
          drag,
          pointer: reorder.pointer.clone(),
          frame: reorder.frame.clone(),
        },
      )
    });
    if let Some(key) = reorder.refocus.lock().take()
      && let Some(card) = reorder.card_element(&key)
    {
      ctx.focus(&card);
    }
    Column::new()
      .id("pipeline-hub-steps-section")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(steps::head(
        ctx,
        props.read.flow.as_ref().map_or(props.count, PipelineFlow::len),
        action,
      ))
      .child(
        Stack::new()
          .id("pipeline-hub-flow-frame")
          .width(Dimension::Pct(100.0))
          .stack_align(StackAlignment::TopStart)
          .ref_element(reorder.frame.clone())
          .child(list)
          .with_children(floating),
      )
  }
}

impl FlowView {
  /// The rail from the start to the end: the stages, or the note that says why there are none to draw.
  fn rows(&self, ctx: &mut Ctx, (props, drive, opens): (&Props, &Drive, &Opens<'_>)) -> Vec<Element> {
    let reorder = &self.reorder;
    let cards = props.read.flow.as_ref().map(Cards::of).unwrap_or_default();
    let mut rows: Vec<Element> = vec![start_row(ctx)];
    match &props.read.flow {
      Ok(flow) => {
        let drag = reorder.drag.get().filter(|_| !props.locked);
        let landing = drag
          .as_ref()
          .and_then(|drag| landing::of(ctx, (flow, &props.current, &cards), drag));
        let draw = Draw {
          opens,
          drive,
          cards: &cards,
          flow,
          locked: props.locked,
          drag: drag.as_ref(),
          landing: landing.as_ref(),
        };
        rows.extend(draw.rows(ctx));
      }
      Err(error) => rows.push(note_row(ctx, super::undrawable(ctx, opens, error))),
    }
    let (label, hint) = words::end(ctx, props.read.ending);
    rows.push(rail::row(
      ctx,
      "pipeline-hub-flow-row-end",
      &Node::End,
      (parts::end(&label, &hint).into(), SpacingSize::Sm),
    ));
    rows
  }
}

/// *Add step*, which steps that draw as one line can be added to; faded while the flow is locked. Steps that cannot be
/// drawn have no head action: their notice offers *Edit steps*.
fn head_action(ctx: &Ctx, drive: &Drive, flow: &PipelineFlow, locked: bool) -> Element {
  let (drive, flow, name) = (drive.clone(), flow.clone(), menu::new_step_name(ctx));
  let button = ActionButton::new(
    "pipeline-hub-add-step".to_owned(),
    ButtonKind::Ghost,
    ctx.t("pipeline_hub.flow.move.addStep"),
  )
  .icon(Icon::Plus)
  .on_click(move || {
    drive.add(name.clone(), None, &flow);
  });
  if locked { button.disabled() } else { button }.into()
}

/// A save began while a card was lifted or its menu was open: they are put down and closed.
fn release(reorder: &Reorder) {
  if reorder.drag.get_untracked().is_some() {
    drag::set_down(reorder);
  }
  if reorder.menu_open.get_untracked() {
    reorder.menu_open.set(false);
  }
}

pub(super) fn start_row(ctx: &Ctx) -> Element {
  rail::row(
    ctx,
    "pipeline-hub-flow-row-start",
    &Node::Start,
    (
      parts::start(
        &ctx.t("pipeline_hub.flow.onStart"),
        &ctx.t("pipeline_hub.flow.onStartHint"),
      )
      .into(),
      SpacingSize::Lg,
    ),
  )
}

/// Alt ↑ and Alt ↓ move the card the keys are on, and Esc sets a lifted card down.
fn keys(drive: &Drive) -> impl Fn(KeyboardEvent) + Send + Sync + use<> {
  let drive = drive.clone();
  move |event: KeyboardEvent| {
    if event.default_prevented() || event.text_input_focused {
      return;
    }
    if event.key == "Escape" {
      if drag::set_down(&drive.reorder) {
        event.prevent_default();
      }
      return;
    }
    let Some(change) = keyed_move(&event) else {
      return;
    };
    if let Some(key) = drive.reorder.focused.get_untracked().0 {
      event.prevent_default();
      drive.apply(&key, &change, true);
    }
  }
}

/// What stands on the line between the start and the end when there is nothing to draw there.
pub(super) fn note_row(ctx: &Ctx, note: Element) -> Element {
  rail::row(ctx, "pipeline-hub-flow-row-note", &Node::Line, (note, SpacingSize::Lg))
}

/// A draft with no steps: a muted line, until *Add step* in the head adds the first one.
pub(super) fn no_steps(ctx: &Ctx) -> Element {
  Row::new()
    .id("pipeline-hub-flow-empty")
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("pipeline_hub.flow.noSteps"))
        .id("pipeline-hub-flow-empty-words")
        .variant(HubText::PanelHint.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}

/// What the stages are drawn from.
struct Draw<'a> {
  locked: bool,
  opens: &'a Opens<'a>,
  drive: &'a Drive,
  cards: &'a Cards,
  flow: &'a PipelineFlow,
  drag: Option<&'a Drag>,
  landing: Option<&'a Landing>,
}

impl Draw<'_> {
  /// The rows between the start and the end: the stages with their gates, the line where a lifted card would land, then
  /// the Deliver card, or *Add Deliver step* in its place.
  fn rows(&self, ctx: &mut Ctx) -> Vec<Element> {
    let mut rows = Vec::new();
    if self.flow.stages.is_empty() {
      rows.push(note_row(ctx, no_steps(ctx)));
    }
    for (index, stage) in self.flow.stages.iter().enumerate() {
      let first = stage.cards.first().map(|card| card.key.clone());
      rows.extend(self.line_before(ctx, first));
      rows.push(self.stage_row(ctx, stage, index + 1 == self.flow.stages.len()));
      for (gate_index, gate) in stage.gates.iter().enumerate() {
        let id = format!("pipeline-hub-flow-row-gate-{index}-{gate_index}");
        let line = parts::gate(
          index * 100 + gate_index,
          &ctx.t("pipeline_hub.flow.gate"),
          &words::gate(ctx, gate, self.flow.deliver.as_ref().map(|deliver| deliver.number)),
          gate,
        );
        rows.push(rail::row(ctx, &id, &Node::Gate, (line.into(), SpacingSize::Lg)));
      }
    }
    rows.extend(self.line_before(ctx, None));
    if let Some(deliver) = &self.flow.deliver {
      rows.push(rail::row(
        ctx,
        "pipeline-hub-flow-row-deliver",
        &Node::Deliver,
        (
          card::deliver(ctx, deliver, self.opens.button(PipelineView::Deliver)),
          SpacingSize::Sm,
        ),
      ));
    } else {
      rows.push(add_deliver::row(ctx, self.opens, &self.drive.edit, self.locked));
    }
    rows
  }

  /// The line of a lifted card that would land just before `before` (at the end for none).
  fn line_before(&self, ctx: &Ctx, before: Option<StepKey>) -> Option<Element> {
    let landing = self.landing?;
    (landing.at == Drop::Before(before) && !matches!(landing.words, Words::Parallel(_)))
      .then(|| landing::line(ctx, &landing.words))
  }

  /// A stage: one step on the line with its own node, or steps that run side by side in their frame; under it what a
  /// lifted card dropped on it would do, or why the last move was refused.
  fn stage_row(&self, ctx: &mut Ctx, stage: &Stage, last: bool) -> Element {
    let reorder = &self.drive.reorder;
    let laid = reorder.stage(stage.cards.iter().map(|card| card.key.clone()));
    let mut elements: Vec<Element> = stage
      .cards
      .iter()
      .zip(laid)
      .map(|(step, (_, element))| {
        let place = if stage.cards.len() == 1 {
          Place::OnTheLine
        } else {
          Place::InGroup
        };
        self.card(ctx, step, place, element)
      })
      .collect();
    let first = stage.cards.first().map_or(0, |step| step.number);
    let room = SpacingSize::Sm;
    let (node, body) = if let [only] = stage.cards.as_slice() {
      let node = if only.runs.needs_you() {
        Node::NotReady(only.number)
      } else {
        Node::Step(only.number)
      };
      (node, elements.remove(0))
    } else {
      let (label, hint) = (
        ctx.t("pipeline_hub.flow.parallel"),
        ctx.t(if stage.cards.len() > 2 {
          "pipeline_hub.flow.parallelAll"
        } else {
          "pipeline_hub.flow.parallelBoth"
        }),
      );
      let group = parts::parallel(
        &format!("pipeline-hub-flow-parallel-{first}"),
        (&label, &hint),
        elements,
      );
      (Node::Parallel, group)
    };
    let mut cell = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xs)
      .child(body);
    for note in self.notes(ctx, stage, last) {
      cell = cell.child(note);
    }
    rail::row(
      ctx,
      &format!(
        "pipeline-hub-flow-row-{}",
        stage.cards.first().map_or(0, |card| card.number)
      ),
      &node,
      (cell.into(), room),
    )
  }

  /// What stands under a stage: what dropping on one of its cards does, why the last move was refused, and the open ⋯
  /// menu of one of its cards.
  fn notes(&self, ctx: &mut Ctx, stage: &Stage, last: bool) -> Vec<Element> {
    let mut notes = Vec::new();
    let has = |key: &StepKey| stage.cards.iter().any(|card| &card.key == key);
    if let Some(Landing {
      at: Drop::OnCard(target),
      words,
    }) = self.landing
      && has(target)
      && let Some((text, refused)) = words.under_card()
    {
      notes.push(landing::card_words(text, refused));
    }
    if let Some(Refused {
      key: Some(key), error, ..
    }) = self.drive.reorder.refused.get().as_ref()
      && has(key)
    {
      notes.push(self.refusal(ctx, key, error));
    }
    if let Some(Refused { key: None, error, .. }) = self.drive.reorder.refused.get().as_ref()
      && last
    {
      let text = ctx.t_args(
        "pipeline_hub.flow.move.refusedAdd",
        [("reason", self.cards.reason(ctx, error, None).to_string())],
      );
      notes.push(landing::card_words(&text, true));
    }
    let menu = Menu {
      drive: self.drive,
      flow: self.flow,
      cards: self.cards,
      opens: None,
    };
    if let Some(open) = self.drive.reorder.menu_for.get().0.filter(|_| !self.locked)
      && let Some(step) = stage.cards.iter().find(|card| card.key == open)
    {
      notes.push(menu::popup(ctx, &menu, step, self.drive.reorder.more_anchor(&step.key)).into());
    }
    notes
  }

  fn refusal(&self, ctx: &Ctx, key: &StepKey, error: &orchester_core::resource::MoveError) -> Element {
    let text = ctx.t_args(
      "pipeline_hub.flow.move.refused",
      [
        ("name", self.cards.name(key).to_string()),
        ("reason", self.cards.reason(ctx, error, Some(key)).to_string()),
      ],
    );
    landing::card_words(&text, true)
  }

  /// The card of `step`, with its handle and ⋯ while the pointer or the keys are on it.
  fn card(&self, ctx: &mut Ctx, step: &StepCard, place: Place, element: lurq::core::ElementRef) -> Element {
    let reorder = &self.drive.reorder;
    let key = step.key.clone();
    let shown = match self.drag {
      _ if self.locked => false,
      Some(drag) => drag.key == key,
      None => {
        reorder.hovered.get().is(&key)
          || reorder.focused.get().is(&key)
          || (reorder.menu_open.get() && reorder.menu_for.get().is(&key))
      }
    };
    let look = match (self.drag, self.landing) {
      (Some(drag), _) if drag.key == key => Look::Lifted,
      (
        _,
        Some(Landing {
          at: Drop::OnCard(target),
          ..
        }),
      ) if *target == key => Look::Target,
      _ => Look::Plain,
    };
    let (enter, leave, gained, lost) = (key.clone(), key.clone(), key.clone(), key.clone());
    let (hovered, left, focused, blurred) = (reorder.clone(), reorder.clone(), reorder.clone(), reorder.clone());
    let open = self.opens.button(PipelineView::Step(step.key.clone())).map(|button| {
      // *Choose another*: the step opens with its Agent picker open.
      let button = match self.opens.pick_agent.clone().filter(|_| step.gone) {
        Some(pick) => button.on_click(move |_: MouseEvent| pick.set(true)),
        None => button,
      };
      Focus::Keyboard
        .apply(button)
        .ref_element(element.clone())
        .on_mouse_enter(move || {
          hovered.hovered.set(Held(Some(enter.clone())));
          hovered.settle_refusal();
        })
        .on_mouse_leave(move || {
          left.hovered.update(|held| {
            if held.is(&leave) {
              *held = Held::default();
            }
          });
          left.settle_refusal();
        })
        .on_focus(move || {
          focused.focused.set(Held(Some(gained.clone())));
          focused.settle_refusal();
        })
        .on_blur(move || {
          blurred.focused.update(|held| {
            if held.is(&lost) {
              *held = Held::default();
            }
          });
          blurred.settle_refusal();
        })
    });
    let fix = Fix::of_card(step).and_then(|fix| self.opens.fix(fix).map(|button| (button, fix.words(ctx))));
    let handle = shown.then(|| drag::handle(ctx, self.drive, step, element));
    let more = (shown && self.drag.is_none()).then(|| menu::more_button(ctx, reorder, step, reorder.more_anchor(&key)));
    card::step(
      ctx,
      step,
      Opening {
        open,
        fix,
        place,
        handle,
        more,
        look,
      },
    )
  }
}
