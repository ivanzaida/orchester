//! What the composer shows of plan-only mode (`12` `SDd2A`, `12d` `y79oJ7`): the *Plan only* switch in its bar after
//! the runs-on chip (`Toggle/Switch` `whZdG`), whose tooltip says what it does and that Alt+P flips it from the
//! message (`oR2hQ`; the design's Shift Tab moves the focus back, so the person chose Alt+P); while it is on, the
//! *Mode* line on top of the box (`y51fw9`): the `hover` fill, radius 6, 6 over
//! and under and 8 at its sides, a 13 `list-checks`, *Plan only* in 12/500 `text_primary` and what it means in 12
//! `text_muted`, 8 apart; and while a plan is being changed, the chip naming it under that line (`Tag chip` `BOcw2`),
//! whose cross stops changing it.

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::display;

use super::Changing;
use crate::{
  screens::orchestrator::{View, composer::PlanParts, send_menu::Press},
  theme::{ExtraColor, ExtraSpacing, IconSize, OrchestratorText},
  ui::{
    chip::TagChip,
    icon::{Icon, icon},
    message::message_text,
    tinted_note::tinted_note,
    toggle::{Toggle, ToggleKind},
    tooltip::Tooltip,
  },
};

/// The composer's parts of plan-only mode for the conversation `view` holds, of the project at `root`.
pub(in crate::screens::orchestrator) fn parts(ctx: &mut Ctx, view: &View, root: &Path) -> PlanParts {
  let plan = &view.plan_only;
  let on = plan.on(view.of.get());
  let flip: Press = {
    let (plan, of, root) = (plan.clone(), view.of.clone(), root.to_path_buf());
    Arc::new(move || {
      if let Some(of) = of.get_untracked() {
        plan.flip(of, plan.on_now(Some(of)), &root);
      }
    })
  };
  let mut over = Vec::new();
  if on {
    over.push(mode_line(ctx));
  }
  // A switch that could not be kept shows as the conversation keeps it, and says why.
  if let Some(why) = plan.failed(view.of.get()) {
    over.push(
      tinted_note(
        "orchestrator-plan-only-error",
        (Icon::CircleAlert, PaletteColor::Danger),
        &message_text(ctx, &why),
      )
      .into(),
    );
  }
  if let Some(changing) = plan.changing.get() {
    let stop = plan.clone();
    over.push(changing_chip(ctx, changing, move || stop.stop_changing()));
  }
  PlanParts {
    switch: switch(ctx, on, flip.clone()),
    over,
    flip,
  }
}

/// The switch, `on` or off; a press flips it as `flip` does. Its tooltip names the keys that flip it from the message:
/// Alt+P (⌥P on macOS), which the person chose over the design's Shift Tab, since Shift+Tab moves the focus back.
fn switch(ctx: &mut Ctx, on: bool, flip: Arc<dyn Fn() + Send + Sync>) -> Element {
  let toggle = Toggle::new(
    "orchestrator-plan-only",
    ToggleKind::Switch,
    on,
    ctx.t("orchestrator.plan.switch"),
  )
  .on_change(move |_| flip());
  let row = Row::new()
    .id("orchestrator-plan-only-switch")
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .child(toggle);
  let keys = ctx.t(if cfg!(target_os = "macos") {
    "orchestrator.plan.switchKeysMac"
  } else {
    "orchestrator.plan.switchKeys"
  });
  Tooltip::new("orchestrator-plan-only", ctx.t("orchestrator.plan.switchTip"))
    .shortcut(keys)
    .wrap(ctx, row)
}

/// The *Mode* line over the message while the switch is on.
fn mode_line(ctx: &Ctx) -> Element {
  Row::new()
    .id("orchestrator-plan-mode")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(ExtraColor::Hover.role())
    .rounded(RadiusSize::Md)
    .child(icon(Icon::ListChecks, IconSize::Base, PaletteColor::TextPrimary).flex_shrink(0.0))
    .child(
      Text::new(&ctx.t("orchestrator.plan.mode"))
        .id("orchestrator-plan-mode-name")
        .variant(OrchestratorText::Mode.role())
        .nowrap(),
    )
    .child(
      Text::new(&ctx.t("orchestrator.plan.modeNote"))
        .id("orchestrator-plan-mode-note")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}

/// The chip of the plan being changed, `changing`; its cross stops changing it, as `stop` does.
fn changing_chip(ctx: &Ctx, changing: Changing, stop: impl Fn() + Send + Sync + 'static) -> Element {
  let words = ctx.t_args(
    "orchestrator.plan.changingChip",
    [
      ("steps", changing.steps.to_string()),
      ("time", display::clock(changing.at)),
    ],
  );
  let chip = TagChip::new("orchestrator-plan-changing", words).removable(ctx.t("orchestrator.plan.stopChanging"), stop);
  Row::new()
    .id("orchestrator-plan-changing-line")
    .width(Dimension::Pct(100.0))
    .child(chip)
    .into()
}
