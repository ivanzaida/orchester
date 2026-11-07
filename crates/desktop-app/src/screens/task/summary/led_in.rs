//! *Led in* among the task's properties (`Led in` `IbtAj` in 09 `SLuUW`, `i687ei` b): the Orchestrator conversation
//! that leads the task, after a 13 `messages-square` in `text_secondary`, in 12 `text_primary` on one line cut to its
//! room with the whole of it in a tooltip, and *· via Webhooks v2* when it leads the task through its module, the tasks
//! without a module or the whole project; a 12 `arrow-up-right` in `text_muted` at the row's end, 6 apart. A press
//! opens that conversation. No row while nobody leads the task. Who leads it is `orchester_workspace`'s
//! (`WatchReads::task_led_in`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::orchestrator::watching::LedIn;

use super::properties::row;
use crate::{
  navigation::Navigation,
  routes::{OrchestratorChat, ProjectPage, Route},
  screens::task::TaskContext,
  theme::{ExtraSpacing, IconSize, TaskSpacing},
  ui::{
    icon::{Icon, icon},
    message::message_text,
    pressable::opening,
    tooltip::Truncated,
  },
};

const ID: &str = "task-property-led-in";

/// The *Led in* row, `task-property-led-in-row`; `None` while nobody leads the task.
pub fn led_in_row(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let LedIn::Chat(leader) = &task.detail.led_in else {
    return None;
  };
  let words = message_text(ctx, &leader.words);
  let project = task.project.open.project.id();
  let route = Route::Project(project, ProjectPage::Orchestrator(OrchestratorChat::Chat(leader.chat)));
  let text = Truncated::new(format!("{ID}-label"), words.clone(), TypographyStyle::Description)
    .color(PaletteColor::TextPrimary)
    .render(ctx);
  let link = opening(Navigation::of(ctx).to(route))
    .id(ID)
    .describe("a11y_name", words)
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(icon(Icon::MessagesSquare, IconSize::Base, PaletteColor::TextSecondary))
    .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(text))
    .child(icon(Icon::ArrowUpRight, IconSize::Sm, PaletteColor::TextMuted));
  Some(row(
    ID,
    &ctx.t("summary.properties.ledIn"),
    link,
    TaskSpacing::PropertyRow.role(),
  ))
}
