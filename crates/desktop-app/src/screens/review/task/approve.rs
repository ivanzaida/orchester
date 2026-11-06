//! What *Approve and merge* asks first while comments wait unsent in the tray (`Review - v2` 09 `mhp3i`, `Dialog`
//! `U5mCzY`, rule 7): *1 comment isn't sent yet*, which comment (its file and lines) or how many, and *Cancel*,
//! *Approve without it* (merges; the comments not sent are discarded) and *Send to Implement* (sends them as the tray
//! does and approves nothing). Its footer is the plain one: three buttons leave no room for *Esc cancels*. With none
//! unsent *Approve and merge* merges at once.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::TypographyStyle},
  components::Text,
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::ReviewMessageId, pull_request::feedback::LineSpan};

use super::{
  Env,
  actions::Write,
  words::{counted, lines},
};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  dialog::{Dialog, Tone},
  icon::Icon,
};

/// The dialog's width (`U5mCzY`).
const DIALOG_WIDTH: f32 = 460.0;

/// A comment not sent yet, in a thread not resolved, and the lines it is about.
pub(super) struct Unsent {
  pub id: ReviewMessageId,
  path: Arc<str>,
  span: LineSpan,
}

/// The comments not sent yet, of threads not resolved.
pub(super) fn unsent(env: &Env) -> Vec<Unsent> {
  env
    .review
    .threads
    .iter()
    .filter(|view| view.thread.resolved_at().is_none())
    .flat_map(|view| {
      let anchor = view.thread.anchor();
      view.thread.unsent().map(|message| Unsent {
        id: message.id(),
        path: Arc::from(anchor.path()),
        span: anchor.span(),
      })
    })
    .collect()
}

/// What the dialog says: the comment's file and lines, or how many comments there are.
fn body(ctx: &Ctx, unsent: &[Unsent]) -> Arc<str> {
  match unsent {
    [only] => ctx.t_args(
      "review_page.approveAsk.bodyOne",
      [
        ("file", only.path.to_string()),
        ("lines", lines(ctx, only.span).to_string()),
      ],
    ),
    several => ctx.t_args(
      "review_page.approveAsk.bodyOther",
      [("count", several.len().to_string())],
    ),
  }
}

/// The dialog, while *Approve and merge* was pressed with comments not sent.
pub(super) fn dialog(ctx: &Ctx, env: &Env) -> Element {
  let unsent = unsent(env);
  if !env.state.approving.get() || unsent.is_empty() {
    return Element::new();
  }
  let count = unsent.len();
  let words = |one: &'static str, other: &'static str| counted(ctx, (one, other), count);
  let (open, writes, head) = (env.state.approving.clone(), env.writes.clone(), env.head.clone());
  let without = {
    let (open, writes) = (open.clone(), writes.clone());
    let ids: Vec<ReviewMessageId> = unsent.iter().map(|comment| comment.id).collect();
    ActionButton::new(
      "review-approve-ask-without",
      ButtonKind::Secondary,
      words(
        "review_page.approveAsk.withoutOne",
        "review_page.approveAsk.withoutOther",
      ),
    )
    .icon(Icon::Check)
    .on_click(move || {
      open.set(false);
      writes.run(Write::Approve(head.clone(), ids.clone()));
    })
  };
  let send = {
    let open = open.clone();
    ActionButton::new(
      "review-approve-ask-send",
      ButtonKind::Primary,
      ctx.t("review_page.approveAsk.send"),
    )
    .icon(Icon::Send)
    .on_click(move || {
      open.set(false);
      writes.run(Write::Send);
    })
  };
  Dialog::new(
    "review-approve-ask",
    Icon::MessageSquare,
    words("review_page.approveAsk.titleOne", "review_page.approveAsk.titleOther"),
    ctx.t("review_page.approveAsk.cancel"),
  )
  .tone(Tone::Info)
  .width(DIALOG_WIDTH)
  .plain_footer()
  .part(
    Text::new(&body(ctx, &unsent))
      .id("review-approve-ask-body")
      .variant(TypographyStyle::Description)
      .width(Dimension::Pct(100.0)),
  )
  .action(without)
  .action(send)
  .render(ctx, &open)
}
