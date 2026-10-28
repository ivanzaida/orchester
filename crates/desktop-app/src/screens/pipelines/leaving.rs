//! *Leave <pipeline>?*, asked in place under the header when leaving would discard edits (`05b` `eCrOM`). While it
//! is asked the page's menus and popovers close: one left open would take the next press outside it, the one on
//! *Leave page*, to close itself, and the page would stay.
use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::Signal,
  node::Element,
};

use crate::{
  navigation::{Navigation, guard::LeaveGuard},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    notice::{Notice, NoticeTone},
  },
};

/// The page's leave guard, provided to the parts that open a menu or a popover.
#[derive(Clone)]
pub struct Leaving(pub LeaveGuard);

impl DevtoolsInspectable for Leaving {}

/// Closes `open` while the page asks whether to leave. Read in a render, it renders again when that is asked.
pub fn close_while_asked(ctx: &mut Ctx, open: &Signal<bool>) {
  let asked = ctx
    .use_context::<Leaving>()
    .is_some_and(|leaving| leaving.0.requested());
  if asked && open.get_untracked() {
    open.set(false);
  }
}

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
