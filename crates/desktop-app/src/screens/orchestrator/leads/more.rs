//! *+N* after the Leads chips (`R8tXP` in `X93D8T` c; `More` in screen A `gTvFf`) and the list it opens (`Leads
//! popover` `Jl9jP` in screen A2 `nF6Ro`, on the board `Orchestrator WIP` `mPw5l`).
//!
//! *+N* is the chips' 22 high round pill in a `border` stroke, its words 11.5 `text_secondary`, and a control: the
//! pointer's cursor, under the pointer and while its list is open the `hover` fill in a `border_strong` stroke, a Tab
//! stop that Enter or Space presses, named for the targets it folds. A press opens or closes its list; Esc and a press
//! outside close it, and so does *+N* going away (everything fits again, or nothing is led).
//!
//! The list hangs 2 under the header, its start at *+N*'s, or moved back just enough to end 8 before the window's
//! edge: 320 wide on `surface_raised` in a 1 `border_strong` stroke, radius md, 6 inside, rows 2 apart. A head line 26
//! high, 8 at its sides, *This conversation leads* in 11.5 `text_muted`; then a row per target the conversation leads,
//! all of them in the order they were kept: 30 high, radius sm, 8 at its sides and 8 between its parts, the chip's 13
//! icon in `text_secondary`, the name (a task by its key) in 12.5 `text_primary`, cut short with an ellipsis, *since
//! 09:14* (or *since Oct 8* for an earlier day) in 11.5 `text_muted`, the free room, and a 12 `x` in `text_muted`,
//! `text_secondary` under the pointer ([`super::stop`]). A row takes the `hover` fill under the pointer; a press on it
//! opens the module or the task as its chip does and closes the list; its `x` stops that subscription as the chip's ×
//! does, opening nothing. A stop that was not applied is said under the rows.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::{CollisionStrategy, Ctx},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Button, Column, Placement, Popup, Row, Text, TextOverflow},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::orchestrator::watch::NamedTarget;
use orchester_workspace::{
  display,
  orchestrator::watching::{ChatLeads, LeadChip},
};

use super::{
  CHIP_HEIGHT, ID, Refused,
  chip::{Hold, route, target_icon, today, under_head},
  slug,
  stop::stop_button,
};
use crate::{
  navigation::Navigation,
  theme::{ControlHeight, ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, OrchestratorText},
  ui::{
    icon::icon,
    message::message_text,
    pressable::{Focus, Press},
  },
};

/// The list's width (`Jl9jP`).
const POPOVER_WIDTH: f32 = 320.0;

/// *+N* and whether its list is open.
pub(super) struct More {
  open: Signal<bool>,
  pill: ElementRef,
}

impl More {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      pill: ctx.element_ref(),
    }
  }

  /// Closes the list: *+N* is gone.
  pub(super) fn close(&self) {
    if self.open.get_untracked() {
      self.open.set(false);
    }
  }

  /// *+N* while `shown` of `leads`' chips leave some folded, with its list while it is open; the list's rows stop
  /// through `hold`, and say why the last stop was `failed`.
  pub(super) fn render(
    &self,
    ctx: &mut Ctx,
    (leads, shown): (&ChatLeads, usize),
    (hold, failed): (&Hold, Option<Refused>),
  ) -> Option<Element> {
    let folded: Vec<String> = leads.targets[shown..]
      .iter()
      .map(|lead| message_text(ctx, &lead.target.name).to_string())
      .collect();
    if folded.is_empty() {
      self.close();
      return None;
    }
    let open = self.open.get();
    let id = format!("{ID}-more");
    let toggle = self.open.clone();
    let pill = Focus::Keyboard
      .apply(Press::run(move || toggle.update(|open| *open = !*open)).button())
      .id(id.clone())
      .ref_element(self.pill.clone())
      .describe(
        "a11y_name",
        ctx.t_args("orchestrator.leads.moreTip", [("targets", folded.join(", "))]),
      )
      .describe("expanded", if open { "true" } else { "false" })
      .height(Dimension::Px(CHIP_HEIGHT))
      .flex_shrink(0.0)
      .padding(Padding::new().left(SpacingSize::Sm).right(SpacingSize::Sm))
      .rounded(ExtraRadius::Full.role())
      .border(Border::inside(
        BorderSize::Sm,
        if open {
          PaletteColor::BorderStrong
        } else {
          PaletteColor::Border
        },
      ))
      .hovered_style(
        Style::new()
          .background(ExtraColor::Hover.role())
          .border_inside(BorderSize::Sm, PaletteColor::BorderStrong),
      )
      .child(
        Text::new(&ctx.t_args("orchestrator.leads.more", [("count", folded.len().to_string())]))
          .variant(OrchestratorText::LeadsChip.role())
          .nowrap(),
      );
    let pill = if open {
      pill.background(ExtraColor::Hover.role())
    } else {
      pill
    };
    let list = open.then(|| self.popover(ctx, (&id, leads), (hold, failed)));
    Some(
      Row::new()
        .flex_shrink(0.0)
        .align_items(Alignment::Center)
        .child(pill)
        .with_children(list)
        .into(),
    )
  }

  /// The list of every target `leads` names, hung 2 under the header from *+N*'s start.
  fn popover(&self, ctx: &mut Ctx, (id, leads): (&str, &ChatLeads), (hold, failed): (&Hold, Option<Refused>)) -> Popup {
    let id = format!("{id}-popover");
    let head = Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ControlHeight::Snug.value()))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .align_items(Alignment::Center)
      .child(
        Text::new(&ctx.t("orchestrator.leads.popover.title"))
          .id(format!("{id}-head"))
          .variant(ExtraTypography::Note.role())
          .nowrap(),
      );
    let rows: Vec<Element> = leads.targets.iter().map(|lead| self.row(ctx, lead, hold)).collect();
    let refused = failed.map(|refused| {
      let reason = message_text(ctx, &refused.reason);
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
        .child(
          Text::new(&ctx.t_args("orchestrator.leads.stopFailed", [("reason", &*reason)]))
            .id(format!("{id}-refused"))
            .variant(ExtraTypography::Note.role())
            .color(PaletteColor::TextSecondary)
            .width(Dimension::Pct(100.0)),
        )
    });
    let panel = Column::new()
      .id(id)
      .width(Dimension::Px(POPOVER_WIDTH))
      .padding(ExtraSpacing::Snug.role())
      .spacing(ExtraSpacing::Tight.role())
      .rounded(RadiusSize::Md)
      .background(PaletteColor::SurfaceRaised)
      .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
      .child(head)
      .with_children(rows)
      .with_children(refused);
    let down = under_head(&hold.head, &self.pill, ExtraSpacing::Tight.value());
    // lurq keeps a popup inside the window, and 8 of clear room after the list keep it 8 from the window's edge. An
    // offset from where *+N* was laid out would not: a position read back may be from before the pass that moved *+N*.
    let framed = Column::new()
      .padding(Padding::new().right(SpacingSize::Sm))
      .child(panel);
    Popup::new(self.pill.clone(), framed)
      .open(self.open.clone())
      .placement(Placement::BottomStart)
      .offset(0.0, down)
      .collision(CollisionStrategy::Clamp)
  }

  /// `lead`'s row: a press opens its target as its chip does and closes the list; its `x` stops it through `hold`.
  fn row(&self, ctx: &mut Ctx, lead: &LeadChip, hold: &Hold) -> Element {
    let target = lead.target.target;
    let id = format!("{ID}-more-{}", slug(target));
    let name = message_text(ctx, &lead.target.name);
    let base = match route(lead) {
      Some(route) => {
        let (open, to) = (self.open.clone(), Navigation::of(ctx).to(route));
        Press::Open(Arc::new(move |at| {
          open.set(false);
          to(at);
        }))
        .button()
      }
      None => Button::empty().align_items(Alignment::Center).focusable(false),
    };
    let named = NamedTarget {
      target,
      name: Some(name.to_string()),
    };
    let stop = stop_button(ctx, &id, (hold, named), (IconSize::Sm, false));
    base
      .id(id.clone())
      .describe("a11y_name", name.clone())
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ControlHeight::Row.value()))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .spacing(SpacingSize::Sm)
      .rounded(RadiusSize::Sm)
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .child(icon(target_icon(target), IconSize::Base, PaletteColor::TextSecondary))
      .child(
        Text::new(&name)
          .id(format!("{id}-name"))
          .variant(OrchestratorText::Line.role())
          .nowrap()
          .text_overflow(TextOverflow::Elipsis)
          .flex_shrink(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .child(
        Text::new(&since(ctx, lead))
          .id(format!("{id}-since"))
          .variant(ExtraTypography::Note.role())
          .nowrap()
          .flex_shrink(0.0),
      )
      .child(Row::new().flex(1.0))
      .child(stop)
      .into()
  }
}

/// *since 09:14*, or *since Oct 8* for an earlier day, as the chip's tooltip says it.
fn since(ctx: &Ctx, lead: &LeadChip) -> Arc<str> {
  if today(lead.since) {
    let time = display::clock(lead.since);
    ctx.t_args("orchestrator.leads.popover.sinceToday", [("time", time.as_str())])
  } else {
    let date = super::super::date(ctx, lead.since);
    ctx.t_args("orchestrator.leads.popover.sinceOn", [("date", &*date)])
  }
}
