//! The agent field beside *Send to Implement* in the tray and beside *Ask the agent to resolve* in the conflict bar
//! (`Review - v2` 12 `IfAmk`, `Agent` in 02 `gSMiI` and 11 `CT6Gc`): a select 196 × 28 that shows the agent profile the
//! step runs with this time, *Builder · claude-opus-5-5*, and starts on the run's own, the profile its pipeline gives
//! the step the run returns to. It opens a menu over it, *Implement runs with*, of the project's published agent
//! profiles (`Agent menu` `BpZlb`): a bot glyph, the name, the model and, on the run's own, *this run's agent*, a check
//! on the chosen one, and the note that the choice is for this time only. Choosing sets the field alone; the plain
//! button beside it sends with that profile. The choice is kept for the run while the page lives, and is the run's own
//! again once its profile is no longer published.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Placement, Popup, Row, Text, TextOverflow},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ResourceId;
use orchester_workspace::tasks::review_threads::{agents::ReviewAgent, page::ReviewPage};

use super::{Env, state::Held};
use crate::{
  theme::{ControlHeight, ExtraSpacing, IconSize, ReviewText},
  ui::{
    icon::{Icon, icon},
    menu::{MenuItem, menu_panel, separator},
    pressable::pressable,
  },
};

/// The field's width (`Agent` `NvDRg` in 02) and its menu's (`Agent menu` `BpZlb`).
const FIELD_WIDTH: f32 = 196.0;
const MENU_WIDTH: f32 = 300.0;

/// Where a field stands: the tray's or the conflict bar's. Each opens its own menu over itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Place {
  Tray,
  Conflict,
}

impl Place {
  /// The element id of the field there; its menu's and rows' ids follow it.
  const fn id(self) -> &'static str {
    match self {
      Self::Tray => "review-tray-agent",
      Self::Conflict => "review-conflict-agent",
    }
  }
}

/// A field's menu: open or not, over the field it belongs to.
#[derive(Clone)]
struct Menu {
  open: Signal<bool>,
  anchor: ElementRef,
}

impl Menu {
  fn new(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ElementRef::default(),
    }
  }
}

/// The agent chosen for the run, shared by both fields: `None` is the run's own.
#[derive(Clone)]
pub(super) struct AgentChoice {
  chosen: Signal<Held<Option<ResourceId>>>,
  tray: Menu,
  conflict: Menu,
}

impl AgentChoice {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      chosen: ctx.signal(Held(None)),
      tray: Menu::new(ctx),
      conflict: Menu::new(ctx),
    }
  }

  const fn menu(&self, place: Place) -> &Menu {
    match place {
      Place::Tray => &self.tray,
      Place::Conflict => &self.conflict,
    }
  }

  /// The profile a send or a resolve runs the step with, read when it is pressed: `None` for the run's own agent.
  pub fn profile(&self, page: &ReviewPage) -> Option<ResourceId> {
    elsewhere(self.chosen.get_untracked().0, page)
  }
}

/// `chosen`, unless it is the run's own agent or no longer one of the published profiles, which run the step as the
/// pipeline gives it.
fn elsewhere(chosen: Option<ResourceId>, page: &ReviewPage) -> Option<ResourceId> {
  let own = page.implement.as_ref()?.own.as_ref()?.id;
  chosen.filter(|chosen| *chosen != own && page.agents.iter().any(|agent| agent.id == *chosen))
}

/// The field at `place`, with its menu; `None` when the run has no agent step to return to.
pub(super) fn field(ctx: &mut Ctx, env: &Env, place: Place) -> Option<Element> {
  let own = env.page.implement.as_ref()?.own.as_ref()?;
  let elsewhere = elsewhere(env.state.agent.chosen.get().0, &env.page);
  let shown = elsewhere
    .and_then(|id| env.page.agents.iter().find(|agent| agent.id == id))
    .unwrap_or(own);
  let menu = env.state.agent.menu(place);
  let id = place.id();
  let trigger = trigger(ctx, (id, menu), shown);
  let panel = panel(ctx, env, (place, own), elsewhere.unwrap_or(own.id));
  Some(
    Column::new()
      .child(trigger)
      // Over the field from its end, 6 clear of it: over its anchor lurq counts the offset away from it.
      .child(
        Popup::new(menu.anchor.clone(), panel)
          .open(menu.open.clone())
          .placement(Placement::TopEnd)
          .offset(0.0, ExtraSpacing::Snug.value()),
      )
      .into(),
  )
}

/// The select's box: the agent's name and model, and the chevron at its end.
fn trigger(ctx: &Ctx, (id, menu): (&str, &Menu), shown: &ReviewAgent) -> Element {
  let open = menu.open.get();
  let toggle = menu.open.clone();
  let label = ctx.t_args(
    "review_page.agentField.value",
    [("name", shown.name.to_string()), ("model", shown.model.to_string())],
  );
  pressable(move || toggle.update(|open| *open = !*open))
    .id(id.to_owned())
    .ref_element(menu.anchor.clone())
    .width(Dimension::Px(FIELD_WIDTH))
    .height(Dimension::Px(ControlHeight::Md.value()))
    .flex_shrink(0.0)
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(
      BorderSize::Sm,
      if open {
        PaletteColor::BorderFocus
      } else {
        PaletteColor::Border
      },
    ))
    .rounded(RadiusSize::Md)
    .describe("a11y_name", ctx.t("review_page.agentField.name"))
    .describe("state", if open { "open" } else { "closed" })
    .child(
      Text::new(&label)
        .id(format!("{id}-label"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextPrimary)
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(icon(
      if open { Icon::ChevronUp } else { Icon::ChevronDown },
      IconSize::Md,
      PaletteColor::TextMuted,
    ))
    .into()
}

/// The menu: its head, the profiles with `current` checked, a line and the note.
fn panel(ctx: &mut Ctx, env: &Env, (place, own): (Place, &ReviewAgent), current: ResourceId) -> Element {
  let id = place.id();
  let menu = env.state.agent.menu(place);
  // The run's own agent stays a choice when its profile is no longer published.
  let listed = env.page.agents.iter();
  let agents: Vec<&ReviewAgent> = if env.page.agents.iter().any(|agent| agent.id == own.id) {
    listed.collect()
  } else {
    std::iter::once(own).chain(listed).collect()
  };
  let rows: Vec<Element> = agents
    .into_iter()
    .enumerate()
    .map(|(at, agent)| {
      let chosen = env.state.agent.chosen.clone();
      let profile = agent.id;
      let detail: Arc<str> = if agent.id == own.id {
        ctx.t_args("review_page.agentField.own", [("model", agent.model.to_string())])
      } else {
        agent.model.clone()
      };
      MenuItem::new(format!("{id}-menu-{at}"), agent.name.clone(), move || {
        chosen.set(Held(Some(profile)));
      })
      .icon(Icon::Bot)
      .detail(detail)
      .tall()
      .chosen(agent.id == current)
      .render(ctx, &menu.open)
    })
    .collect();
  menu_panel(&format!("{id}-menu"))
    .width(Dimension::Px(MENU_WIDTH))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding(
          Padding::new()
            .top(ExtraSpacing::Snug.role())
            .left(SpacingSize::Sm)
            .right(SpacingSize::Sm)
            .bottom(SpacingSize::Xs),
        )
        .child(
          Text::new(&ctx.t("review_page.agentField.heading"))
            .variant(ReviewText::MenuHeading.role())
            .nowrap(),
        ),
    )
    .with_children(rows)
    .child(separator(ctx))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding(
          Padding::new()
            .top(SpacingSize::Xs)
            .left(SpacingSize::Sm)
            .right(SpacingSize::Sm)
            .bottom(ExtraSpacing::Snug.role()),
        )
        .child(
          Text::new(&ctx.t("review_page.agentField.note"))
            .id(format!("{id}-note"))
            .variant(TypographyStyle::Caption)
            .width(Dimension::Pct(100.0)),
        ),
    )
    .into()
}
