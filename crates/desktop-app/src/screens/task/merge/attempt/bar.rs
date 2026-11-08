//! The merge bar (`Merge bar` in 13a `i0K3xB`, as 11's conflict bar `DDrv4`): under the page's body, on
//! `surface_raised` in a `border` outline (radius 6, 16 at its sides, 10 over and under), a 15 glyph, a title in 13/500
//! with what it means for the run in 12 `text_muted` under it, and at its end the page's actions: *Try again*; *Try
//! now* beside *Stop retrying*; *Record merge*; *Ask the agent to resolve* beside *Resolve in editor* and the agent
//! field; *Put files back* beside *Open folder* and a locked *Try again*. A merged run has no bar, and none shows while
//! the page is read.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::integration::attempt::{AttemptOutcome, MergeAsker};
use orchester_workspace::tasks::merge_attempts::{page::AttemptPage, read::Retrying};

use super::{
  AttemptScreen, Shown,
  actions::{Actions, Busy, open_folder},
  conflicts::Conflicts,
  head::{Next, Place, countdown},
  retry::try_again,
  standing::{HELD, Standing},
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
  theme::{AttemptText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// What a bar says: its glyph, its title (a countdown while Orchester waits) and its words.
struct Words {
  glyph: (Icon, PaletteColor),
  title: Element,
  body: Arc<str>,
}

/// The bar around `words` and its `actions`.
fn frame(words: Words, body_id: &str, actions: Vec<Element>) -> Element {
  Row::new()
    .id("merge-attempt-bar")
    .width(Dimension::Pct(100.0))
    .flex_shrink(0.0)
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(icon(words.glyph.0, IconSize::Nav, words.glyph.1))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(words.title)
        .child(
          Text::new(&words.body)
            .id(body_id.to_owned())
            .variant(AttemptText::BarBody.role())
            .width(Dimension::Pct(100.0)),
        ),
    )
    .with_children(actions)
    .into()
}

/// A title that does not count down.
fn title(text: &str) -> Element {
  Text::new(text)
    .id("merge-attempt-bar-title")
    .variant(ExtraTypography::Strong.role())
    .width(Dimension::Pct(100.0))
    .into()
}

impl AttemptScreen {
  /// The bar of how the last attempt left the run; `None` once it merged.
  pub(super) fn bar(
    &self,
    ctx: &mut Ctx,
    shown: &Shown<'_>,
    standing: Standing,
    (actions, conflicts): (&Actions, Option<&Conflicts<'_>>),
  ) -> Option<Element> {
    if standing.is_merged() {
      return None;
    }
    let mut words = self.words(ctx, shown, standing);
    // What keeps the agent from being asked takes the body's place: nothing else says it.
    let refusal = conflicts
      .filter(|_| standing == Standing::Conflicts)
      .and_then(|conflicts| conflicts.stuck(ctx));
    let body_id = if refusal.is_some() {
      "merge-attempt-bar-reason"
    } else {
      "merge-attempt-bar-body"
    };
    if let Some(reason) = refusal {
      words.body = reason;
    }
    let buttons = match (standing, conflicts) {
      (Standing::Conflicts, Some(conflicts)) => conflicts.actions(ctx),
      (Standing::Retrying(retrying), _) => retrying_actions(ctx, retrying, actions),
      (Standing::AwaitingApproval, _) => vec![self.open_review(ctx, shown)],
      (Standing::NotRecorded, _) => vec![self.record_button(ctx, actions)],
      (Standing::LeftChanged, _) if !shown.page.left_changed.is_empty() => self.left_actions(ctx, shown, actions),
      _ => vec![self.try_again(ctx, shown, actions)],
    };
    Some(frame(words, body_id, buttons))
  }

  /// The glyph, title and words of the bar for `standing`.
  fn words(&self, ctx: &mut Ctx, shown: &Shown<'_>, standing: Standing) -> Words {
    let base = shown.base();
    let page = shown.page;
    let (glyph, color) = standing.page_glyph();
    let simple = |ctx: &Ctx, glyph: (Icon, PaletteColor), key: &str| Words {
      glyph,
      title: title(&ctx.t("merges.page.bar.nothingMerged")),
      body: ctx.t_args(key, [("branch", base)]),
    };
    match standing {
      Standing::Retrying(retrying) => retrying_words(ctx, page, retrying, (glyph, color)),
      Standing::AwaitingApproval => Words {
        glyph: (glyph, color),
        title: title(&ctx.t("merges.page.status.awaitingApproval")),
        body: ctx.t("merges.page.bar.awaitingBody"),
      },
      Standing::Conflicts => conflicts_words(ctx, page, base, (glyph, color)),
      Standing::NotRecorded => Words {
        glyph: (Icon::Database, PaletteColor::Warning),
        title: title(&ctx.t_args("merges.page.bar.notRecorded", [("branch", base)])),
        body: ctx.t("merges.page.bar.notRecordedBody"),
      },
      Standing::LeftChanged if !page.left_changed.is_empty() => {
        let key = if page.left_changed.len() == 1 {
          "merges.page.bar.leftOne"
        } else {
          "merges.page.bar.leftOther"
        };
        Words {
          glyph: (glyph, color),
          title: title(&ctx.t_args(key, [("count", page.left_changed.len().to_string())])),
          body: ctx.t_args("merges.page.bar.leftBody", [("branch", base)]),
        }
      }
      Standing::LeftChanged => simple(ctx, (glyph, color), "merges.page.bar.putBackBody"),
      Standing::QueueStopped => Words {
        glyph: (glyph, color),
        title: title(&ctx.t("merges.page.bar.queueStopped")),
        body: ctx.t("merges.page.bar.queueStoppedBody"),
      },
      Standing::GitStopped => simple(ctx, (glyph, color), "merges.page.bar.gitStoppedBody"),
      Standing::Interrupted => simple(ctx, (glyph, color), "merges.page.bar.interruptedBody"),
      Standing::Moved => simple(ctx, (glyph, color), "merges.page.bar.movedBody"),
      Standing::NotMerged | Standing::Merged | Standing::MergedOutside => {
        let held = page.last().is_some_and(|last| last.attempt.cause() == Some(HELD));
        simple(
          ctx,
          (glyph, color),
          if held {
            "merges.page.bar.heldBody"
          } else {
            "merges.page.bar.refusedBody"
          },
        )
      }
    }
  }

  /// A primary action's button: pressed through `press` while nothing else of the page's is under way, loading while
  /// its own is, disabled while another's is.
  fn pressed(&self, button: ActionButton, mine: Busy, press: Box<dyn Fn() + Send + Sync>) -> Element {
    match self.busy.get() {
      Busy::Idle => button.on_click(press).into(),
      current if current == mine => button.loading().into(),
      _ => button.disabled().into(),
    }
  }

  fn record_button(&self, ctx: &Ctx, actions: &Actions) -> Element {
    let actions = actions.clone();
    self.pressed(
      ActionButton::new(
        "merge-attempt-record-merge",
        ButtonKind::Primary,
        ctx.t("merges.action.recordMerge"),
      )
      .icon(Icon::Database),
      Busy::Recording,
      Box::new(move || actions.record_merge()),
    )
  }

  /// *Try again*: puts a run the merge queue stopped back in its place there, else merges the run once more.
  fn try_again(&self, ctx: &mut Ctx, shown: &Shown<'_>, actions: &Actions) -> Element {
    let button = ActionButton::new(
      "merge-attempt-try-again",
      ButtonKind::Primary,
      ctx.t("merges.action.tryAgain"),
    )
    .icon(Icon::RotateCcw);
    if shown.page.queue_stopped {
      let actions = actions.clone();
      return self.pressed(button, Busy::Requeueing, Box::new(move || actions.requeue()));
    }
    try_again(ctx, shown.project, button, &shown.page.run)
  }

  /// *Open in Review*, where the merge queue waits for the person's approval.
  fn open_review(&self, ctx: &mut Ctx, shown: &Shown<'_>) -> Element {
    let review = Route::Project(
      shown.project.open.project.id(),
      ProjectPage::TaskReview(shown.page.run.key.clone()),
    );
    ActionButton::new(
      "merge-attempt-open-review",
      ButtonKind::Primary,
      ctx.t("merges.page.openReview"),
    )
    .icon(Icon::ArrowUpRight)
    .opens(Navigation::of(ctx).to(review))
    .into()
  }

  /// *Open folder*, *Try again* locked until the files match the base branch, and *Put files back*.
  fn left_actions(&self, ctx: &mut Ctx, shown: &Shown<'_>, actions: &Actions) -> Vec<Element> {
    let folder = shown.project.open.repo_root.clone();
    let open = ActionButton::new(
      "merge-attempt-open-folder",
      ButtonKind::Ghost,
      ctx.t("merges.page.openFolder"),
    )
    .icon(Icon::FolderOpen)
    .on_click(move || open_folder("open-checkout", &folder));
    let locked = ActionButton::new(
      "merge-attempt-try-again",
      ButtonKind::Ghost,
      ctx.t("merges.action.tryAgain"),
    )
    .icon(Icon::RotateCcw)
    .unavailable(ctx.t_args("merges.page.tryAgainWaits", [("branch", shown.base())]));
    let actions = actions.clone();
    let put_back = self.pressed(
      ActionButton::new(
        "merge-attempt-put-back",
        ButtonKind::Primary,
        ctx.t("merges.action.putFilesBack"),
      )
      .icon(Icon::Undo2),
      Busy::PuttingBack,
      Box::new(move || actions.put_files_back()),
    );
    vec![open.into(), locked.into(), put_back]
  }
}

/// *Stop retrying*, and *Try now* while Orchester waits for its next try.
fn retrying_actions(ctx: &Ctx, retrying: Retrying, actions: &Actions) -> Vec<Element> {
  let stop = actions.clone();
  let mut buttons: Vec<Element> = vec![
    ActionButton::new(
      "merge-attempt-stop-retrying",
      ButtonKind::Ghost,
      ctx.t("merges.page.stopRetrying"),
    )
    .icon(Icon::CircleStop)
    .on_click(move || stop.stop_retrying())
    .into(),
  ];
  if retrying.next.is_some() {
    let now = actions.clone();
    buttons.push(
      ActionButton::new(
        "merge-attempt-try-now",
        ButtonKind::Primary,
        ctx.t("merges.action.tryNow"),
      )
      .icon(Icon::RotateCcw)
      .on_click(move || now.try_now())
      .into(),
    );
  }
  buttons
}

/// What the bar says while Orchester tries again: when it will, counting down, or that the merge queue will.
fn retrying_words(ctx: &mut Ctx, page: &AttemptPage, retrying: Retrying, glyph: (Icon, PaletteColor)) -> Words {
  let last = page.last().map_or(MergeAsker::Person, |last| last.attempt.asked_by());
  let (title, body) = if retrying.next.is_some() {
    let next = Next {
      last,
      at: retrying.next,
      place: Place::Bar,
    };
    (countdown(ctx, next), ctx.t("merges.page.bar.retryingBody"))
  } else {
    (
      title(&ctx.t("merges.page.status.queueRetries")),
      ctx.t("merges.page.bar.queueRetriesBody"),
    )
  };
  Words { glyph, title, body }
}

/// What the bar says of a merge that conflicts: how many files conflict with the base branch.
fn conflicts_words(ctx: &Ctx, page: &AttemptPage, base: &str, glyph: (Icon, PaletteColor)) -> Words {
  let count = match page.last().map(|last| last.attempt.outcome()) {
    Some(AttemptOutcome::Conflicts { files }) => files.len(),
    _ => 0,
  };
  let key = if count == 1 {
    "merges.page.bar.conflictsOne"
  } else {
    "merges.page.bar.conflictsOther"
  };
  Words {
    glyph,
    title: title(&ctx.t_args(key, [("count", count.to_string()), ("branch", base.to_owned())])),
    body: ctx.t_args("merges.page.bar.conflictsBody", [("branch", base)]),
  }
}
