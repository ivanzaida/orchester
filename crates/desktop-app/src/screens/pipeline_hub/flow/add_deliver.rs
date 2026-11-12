//! *Add Deliver step* (`Step 06` `PgZjO` in `Pipelines · v2` P15b `uDfc0`): where the Deliver card would stand in the
//! flow of a pipeline without one, always drawn, a card on the line with a plus in its node. Pressing it adds a Deliver
//! step to the edit with the defaults (Approve to merge, a merge commit, Check re-run) and opens it in the panel
//! (P15 `nPaMj`); nothing is stored until *Save*. While a save is under way it is faded and does nothing, as *Add
//! step*.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, HitTestBehavior, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::DeliverStep;

use super::{
  Opens,
  rail::{self, Node},
};
use crate::{
  routes::{PipelineView, ProjectPage, Route},
  screens::pipeline_hub::edit::Edit,
  theme::{DISABLED_OPACITY, ExtraColor, FlowText, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::{Focus, Press},
  },
};

const ID: &str = "pipeline-hub-flow-add-deliver";

/// The row of the rail that holds the card.
pub(super) fn row(ctx: &Ctx, opens: &Opens<'_>, edit: &Edit, locked: bool) -> Element {
  rail::row(
    ctx,
    "pipeline-hub-flow-row-add-deliver",
    &Node::AddDeliver,
    (card(ctx, opens, edit, locked), SpacingSize::Sm),
  )
}

/// The card (`Add Deliver step` `bTEn7`): a `border_strong` outline, radius `md`, 8 over and under and 12 at the sides;
/// a plus and *Add Deliver step*, over what runs end in without one.
fn card(ctx: &Ctx, opens: &Opens<'_>, edit: &Edit, locked: bool) -> Element {
  let (edit, navigation) = (edit.clone(), opens.navigation.clone());
  let deliver = Route::Project(
    opens.project.open.project.id(),
    ProjectPage::Pipeline(opens.pipeline, PipelineView::Deliver),
  );
  let press = Press::run(move || {
    if locked {
      return;
    }
    edit.change(|current| {
      if current.deliver().is_none() {
        current.set_deliver(Some(DeliverStep::default()));
      }
    });
    navigation.go(deliver.clone());
  });
  let top = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Plus, IconSize::Md, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t("pipeline_hub.deliver.add"))
        .id(format!("{ID}-name"))
        .variant(FlowText::Name.role())
        .nowrap(),
    );
  Focus::Keyboard
    .apply(press.button())
    .id(ID)
    .describe("locked", locked.to_string())
    .hit_test(if locked {
      HitTestBehavior::None
    } else {
      HitTestBehavior::Auto
    })
    .opacity(if locked { DISABLED_OPACITY } else { 1.0 })
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .align_items(Alignment::Start)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Xs)
        .child(top)
        .child(
          Text::new(&ctx.t("pipeline_hub.deliver.addLine"))
            .id(format!("{ID}-line"))
            .variant(FlowText::Line.role())
            .width(Dimension::Pct(100.0)),
        ),
    )
    .into()
}
