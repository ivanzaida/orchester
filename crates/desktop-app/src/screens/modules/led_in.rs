//! *Led in* on a module's Summary properties (`Orchestrator watch · redesign`, `i687ei` a, `R2vLzt`): the Orchestrator
//! conversation that leads the module, through its own subscription or the whole project's
//! (`orchester_workspace::orchestrator::watching::LedIn`), as a link that opens it: a 24 high box in a `border` stroke,
//! radius 6, 8 at its sides, a 12 `messages-square` in `text_secondary` 6 before *«title» ↗* in 12 `text_primary`, cut
//! to one line with the whole of it in a tooltip, the `hover` fill under the pointer. While nobody leads it the row
//! reads *Nobody* in `text_muted`: its events go to the usual conversation.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::Text,
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::orchestrator::watching::LedIn;

use super::properties::property;
use crate::{
  navigation::Navigation,
  routes::{OrchestratorChat, ProjectPage, Route},
  theme::{ControlHeight, ExtraColor, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    message::message_text,
    pressable::Press,
    tooltip::Truncated,
  },
};

const ID: &str = "module-property-led-in";

/// The *Led in* row of a module of `project`, led as `led_in` says.
pub(super) fn row(ctx: &mut Ctx, project: ProjectId, led_in: &LedIn) -> Element {
  let key = ctx.t("modules.properties.ledIn");
  let value = match led_in {
    LedIn::Nobody => Text::new(&message_text(ctx, &led_in.message()))
      .id(format!("{ID}-value"))
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextMuted)
      .into(),
    LedIn::Chat(leader) => {
      let words = ctx.t_args(
        "modules.properties.ledInLink",
        [("leader", message_text(ctx, &leader.words).to_string())],
      );
      let route = Route::Project(project, ProjectPage::Orchestrator(OrchestratorChat::Chat(leader.chat)));
      let text = Truncated::new(format!("{ID}-value"), words, TypographyStyle::Description)
        .color(PaletteColor::TextPrimary)
        .render(ctx);
      Press::Open(Navigation::of(ctx).to(route))
        .button()
        .id(format!("{ID}-link"))
        .describe("chat", leader.chat.to_string())
        .height(Dimension::Px(ControlHeight::Sm.value()))
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0))
        .padding(Padding::new().left(SpacingSize::Sm).right(SpacingSize::Sm))
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .rounded(RadiusSize::Md)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .hovered_style(Style::new().background(ExtraColor::Hover.role()))
        .child(icon(Icon::MessagesSquare, IconSize::Sm, PaletteColor::TextSecondary))
        .child(text)
        .into()
    }
  };
  property((ID, &key), value, None)
}
