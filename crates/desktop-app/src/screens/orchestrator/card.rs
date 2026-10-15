//! What the Orchestrator's proposal and action cards share (`Action card` `nQIhx`, the proposals of `b55NqO`,
//! `UKgrR`, `Ee2JE`): the raised frame, radius 10, 12 over and under and 14 at its sides, its parts 10 apart; the
//! head with a 14 icon, the title (12.5/600), a chip and, at its end, the state's 12 icon and words; a note in 11.5
//! beside its 12 icon; and the person's decision on a proposal, taken off the event loop.

use std::sync::Arc;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
    i18n::I18n,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  messages::{self, Message},
  orchestrator::{
    proposal::{self, Decision},
    tools::ChatScope,
  },
};

use crate::{
  mcp::domain,
  services::Services,
  theme::{ConversationText, ExtraSpacing, ExtraTypography, IconSize},
  ui::icon::{Icon, icon},
};

/// The card's frame, named `id`, its outcome told to tools.
pub(super) fn frame(id: &str, outcome: &'static str) -> Column {
  Column::new()
    .id(id.to_owned())
    .describe("outcome", outcome)
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Section.role(), SpacingSize::Md))
    .spacing(ExtraSpacing::Loose.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
}

/// Where a card stands, at the end of its head: its icon in its color, and its words.
pub(super) struct State {
  pub(super) glyph: Icon,
  pub(super) color: PaletteColor,
  pub(super) words: Arc<str>,
}

/// The head of card `id`: its icon and title, a chip after the title, and its state at the end.
pub(super) fn head(id: &str, (glyph, title): (Icon, &str), chip: Option<Element>, state: State) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Md, PaletteColor::TextSecondary))
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(ConversationText::ResultTitle.role())
        .flex_shrink(1.0),
    )
    .with_children(chip)
    .child(Spacer::new().flex(1.0))
    .child(
      Row::new()
        .id(format!("{id}-state"))
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center)
        .child(icon(state.glyph, IconSize::Sm, state.color))
        .child(
          Text::new(&state.words)
            .variant(ExtraTypography::Note.role())
            .color(PaletteColor::TextSecondary)
            .nowrap(),
        ),
    )
}

/// A note of card `id` (`{id}-{suffix}`): `text` in `color` beside its icon in the same color.
pub(super) fn note(id: &str, suffix: &str, (glyph, color): (Icon, PaletteColor), text: &str) -> Row {
  Row::new()
    .id(format!("{id}-{suffix}"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Start)
    .child(icon(glyph, IconSize::Sm, color.clone()))
    .child(
      Text::new(text)
        .variant(ExtraTypography::Note.role())
        .color(color)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}

/// A decision to take on a proposal.
#[derive(Clone)]
pub(super) struct Deciding {
  pub(super) scope: ChatScope,
  pub(super) seq: i64,
  pub(super) decision: Decision,
}

impl DevtoolsInspectable for Deciding {}

pub(super) type DecideAction = FutureAction<Deciding, (), Message>;

/// The person's decisions on a card's proposal, carried out through the domain tools of the session. Each is spawned
/// on the session's runtime when it is taken, so it is carried out and its outcome kept even when the card goes away
/// (another tab, another page) before it ends; the card only waits for it.
pub(super) fn decide_action(ctx: &mut Ctx) -> DecideAction {
  let services = Services::of(ctx);
  let i18n = ctx.i18n().clone();
  ctx.future_action(move |deciding: Deciding| {
    let decided = services.runtime.spawn(decide(services.clone(), i18n.clone(), deciding));
    async move { decided.await.map_err(|_| messages::interrupted())? }
  })
}

/// Takes `deciding` through the domain tools of `services`, explaining a refusal in the words of `i18n`.
async fn decide(services: Services, i18n: I18n, deciding: Deciding) -> Result<(), Message> {
  let tools = domain::on(&services);
  let translate = domain::translate(i18n);
  proposal::decide(
    (&tools, &translate, &services.runners),
    &deciding.scope,
    deciding.seq,
    deciding.decision,
  )
  .await
}
