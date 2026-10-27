//! *Leave <pipeline>?*, asked in place under the header when leaving would discard edits (`05b` `eCrOM`).
use lurq::{app::ctx::Ctx, node::Element};

use crate::{
  navigation::{Navigation, guard::LeaveGuard},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    notice::{Notice, NoticeTone},
  },
};

pub fn question(ctx: &mut Ctx, guard: &LeaveGuard, writing: bool, name: &str) -> Option<Element> {
  if !guard.requested() {
    return None;
  }
  let navigation = Navigation::of(ctx);
  let (leave, keep) = (guard.clone(), guard.clone());
  let confirm = ActionButton::new("pipeline-leave-confirm", ButtonKind::Primary, ctx.t("pipelines.leave"))
    .icon(Icon::LogOut)
    .on_click(move || leave.leave(&navigation));
  Some(
    Notice::new(
      "pipeline-leave-question",
      NoticeTone::Warning,
      ctx.t_args("pipelines.leaveNamed", [("name", name)]),
    )
    .stacked()
    .flat()
    .body(ctx.t("pipelines.leaveBody"))
    .action(if writing { confirm.disabled() } else { confirm })
    .action(
      ActionButton::new("pipeline-leave-keep", ButtonKind::Ghost, ctx.t("pipelines.keep"))
        .on_click(move || keep.keep_editing()),
    )
    .into(),
  )
}
