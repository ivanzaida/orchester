//! The line under the toolbar about the agent of the tab shown (`Activity` `WTzV9`): while it acts, a blue dot,
//! *Builder is acting in this tab* and what it did last (*· typed 8 in Max attempts*), and at the line's end that the
//! person's input makes it wait (`02 · Agent is acting` `UKAkY`); while the person's input holds it, an amber hand,
//! *You're using this tab* and how long it waits (`02b · You use the tab` `k7xvx`). There is no take-over control:
//! using the page is taking over.

use std::{sync::Arc, time::Instant};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use crate::{
  browser::{
    model::{ACTING_FOR, Owner, StripTab},
    state::Showing,
  },
  theme::{BrowserSpacing, ExtraRadius, ExtraTypography, IconSize},
  ui::icon::{Icon, icon},
};

/// The line's height (`Activity` `WTzV9`) and its dot.
const LINE_HEIGHT: f32 = 34.0;
const DOT: f32 = 6.0;

/// The line for the tab shown, while its agent acts or waits for the person.
pub fn line(ctx: &mut Ctx, showing: &Showing) -> Option<Element> {
  let tab = showing.shown().and_then(|tab| showing.strip.find(tab))?;
  if !matches!(tab.owner, Owner::Run { .. }) {
    return None;
  }
  let view = showing.view.as_ref()?;
  let now = Instant::now();
  let agent = agent_name(ctx, tab);
  if showing.marks.holds(&tab.id, now) {
    return Some(frame(
      "using",
      icon(Icon::Hand, IconSize::Md, PaletteColor::Warning).into(),
      (
        ctx.t("browser.activity.using"),
        ctx.t_args("browser.activity.waits", [("agent", agent.as_ref())]),
      ),
      None,
    ));
  }
  let acting = view
    .acting(&tab.id)
    .filter(|acting| now.saturating_duration_since(acting.at) < ACTING_FOR)?;
  let dot = Row::new()
    .width(Dimension::Px(DOT))
    .height(Dimension::Px(DOT))
    .rounded(ExtraRadius::Full.role())
    .background(PaletteColor::Info);
  Some(frame(
    "acting",
    dot.into(),
    (
      ctx.t_args("browser.activity.acting", [("agent", agent.as_ref())]),
      ctx.t_args("browser.activity.detail", [("summary", acting.action.summary.as_str())]),
    ),
    Some(ctx.t("browser.activity.hint")),
  ))
}

/// The name an agent's tab is said to be used by: its step's agent, else its task's key.
pub fn agent_name(ctx: &Ctx, tab: &StripTab) -> Arc<str> {
  match &tab.owner {
    Owner::Run { known: Some(known), .. } => known.agent.clone().unwrap_or_else(|| known.key.clone()),
    Owner::Run { known: None, .. } | Owner::Person => ctx.t("browser.activity.someAgent"),
  }
}

fn frame(state: &str, mark: Element, (title, detail): (Arc<str>, Arc<str>), hint: Option<Arc<str>>) -> Element {
  Row::new()
    .id("browser-activity")
    .describe("state", state.to_owned())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(LINE_HEIGHT))
    .flex_shrink(0.0)
    .padding(Padding::horizontal(BrowserSpacing::Activity.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(mark)
    .child(
      Text::new(&title)
        .id("browser-activity-title")
        .variant(TypographyStyle::Label)
        .nowrap(),
    )
    .child(
      Text::new(&detail)
        .id("browser-activity-detail")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .child(Row::new().flex(1.0))
    .with_children(hint.map(|hint| {
      Text::new(&hint)
        .id("browser-activity-hint")
        .variant(ExtraTypography::Note.role())
        .nowrap()
    }))
    .into()
}
