//! The pull request's CI in the run's Conversation (11 `MK7l7`, 11b `X8xYZ`, 11c `NMHzc`, the event lines of 11d
//! `kLZUJ`), among its rows by time (`orchester_workspace::tasks::timeline::delivery::ci_lines`): a line for each Fix
//! CI sent (*CI failed on #48 · lint, test · Fix CI sent to Builder at 15:44*, in `danger`) or stopped at its limit (in
//! `warning`), and at the end how CI stands now: *CI running on #48*, *CI passed on #48*, *CI not read on #48 · … ·
//! Check again*, or, for a failure no Fix CI answered, a card (`danger` stroke) with the failing checks, each with its
//! log's first line, *Fix CI*, *Open checks on GitHub* and whether *Fix CI automatically* is on. A failure the switch
//! answered keeps its card, with the send and *Turn off for this run* in place of *Fix CI*.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  pull_request::ci::{
    CiCheck,
    fix::{AutoFix, FixEvent, FixEventKind, FixedBy, failure_of, is_failing},
  },
  time::Timestamp,
};
use orchester_workspace::{
  display::clock,
  pull_requests::ci::PullRequestCi,
  tasks::{
    detail::{RunView, TaskDetail},
    timeline::delivery::{CiLine, DeliveryLine, ci_lines},
  },
  workspace::OpenProject,
};

use super::{
  Look, ShownCi,
  actions::{AutoRequest, CiActions, open_page},
  card::{FixFor, fix_button},
  checks::{self, Place},
  words,
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskView},
  services::Services,
  theme::{ConversationSpacing, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// What the Conversation's CI parts read: the run's pull request with its CI, and the task's project and state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CiShare {
  pub shown: ShownCi,
  pub project: OpenProject,
  pub lost: bool,
}

/// The CI lines of `run` with `shown`, oldest first: the line of the Fix CI a card shows instead goes, and so does the
/// line of how CI stands while a Fix CI by hand answered it, which its own line says.
pub fn lines(run: &RunView, shown: &ShownCi, now: Timestamp) -> Vec<DeliveryLine> {
  let at = shown.read().map_or(now, |read| read.read_at);
  let mut lines = ci_lines(&run.ci_fixes, (shown.number(), &shown.found.ci), at);
  if shown.look(&run.ci_fixes) != Look::Fixing {
    return lines;
  }
  let answer = shown
    .read()
    .and_then(|read| failure_of(&read.checks))
    .and_then(|failure| run.ci_fixes.sent_for(&failure).cloned());
  let Some(answer) = answer else {
    return lines;
  };
  let automatic = matches!(
    answer.kind,
    FixEventKind::Sent {
      by: FixedBy::Automatically { .. },
      ..
    }
  );
  lines.retain(|line| match &line.kind {
    orchester_workspace::tasks::timeline::delivery::DeliveryKind::Ci(CiLine::Fix(event)) => {
      !automatic || *event != answer
    }
    orchester_workspace::tasks::timeline::delivery::DeliveryKind::Ci(CiLine::Now { .. }) => automatic,
    _ => true,
  });
  lines
}

/// The row of the CI line `line`, `id` its element's.
pub fn line(ctx: &mut Ctx, (share, detail): (Option<&CiShare>, &TaskDetail), id: &str, line: &CiLine) -> Element {
  let (Some(share), Some(run)) = (share, detail.run.as_ref()) else {
    return Column::new().id(id.to_owned()).into();
  };
  let agent = words::agent(ctx, detail, share.lost);
  match line {
    CiLine::Fix(event) => fix_line(ctx, (share, id), event, &agent),
    CiLine::Now { .. } => now(ctx, (share, detail, run), id, &agent),
  }
}

/// A quiet line of the chat, indented to its messages' text: a 13 glyph in `tone`, the words in 12 in `tone`, and
/// a link at its end.
fn event_line(id: &str, (glyph, tone): (Icon, PaletteColor), text: &str, link: Option<Element>) -> Element {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().left(ConversationSpacing::ApprovalRow.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, tone.clone()))
    .child(
      Text::new(text)
        .id(format!("{id}-text"))
        .variant(TypographyStyle::Description)
        .color(tone)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(link)
    .into()
}

/// A link at a line's end: its words and `↗`, opening `url`.
fn link(ctx: &mut Ctx, id: &str, words: &str, url: Arc<str>) -> Element {
  ActionButton::new(format!("{id}-link"), ButtonKind::Ghost, words)
    .after(Icon::ArrowUpRight)
    .small()
    .tight()
    .on_click(open_page(&Services::of(ctx), url))
    .into()
}

/// *CI failed on #48 · lint, test (ubuntu-latest)*.
fn failed_words(ctx: &Ctx, event: &FixEvent) -> Arc<str> {
  ctx.t_args(
    "ci.event.failed",
    [
      ("number", event.number.to_string()),
      ("names", event.failing.join(&ctx.t("task.listSeparator"))),
    ],
  )
}

/// A Fix CI sent, by hand or automatically, or the switch stopping at its limit.
fn fix_line(ctx: &mut Ctx, (share, id): (&CiShare, &str), event: &FixEvent, agent: &Arc<str>) -> Element {
  let checks_page: Arc<str> = format!("{}/checks", share.shown.found.pull_request.url()).into();
  let failed = failed_words(ctx, event).to_string();
  let (glyph, text) = match event.kind {
    FixEventKind::Sent { by: FixedBy::Hand, .. } => (
      (Icon::CircleX, PaletteColor::Danger),
      ctx.t_args(
        "ci.event.sent",
        [
          ("failed", failed),
          ("agent", agent.to_string()),
          ("time", clock(event.at)),
        ],
      ),
    ),
    FixEventKind::Sent {
      by: FixedBy::Automatically { count, limit },
      ..
    } => (
      (Icon::CircleX, PaletteColor::Danger),
      ctx.t_args(
        "ci.event.sentAuto",
        [
          ("failed", failed),
          ("count", count.to_string()),
          ("limit", limit.get().to_string()),
        ],
      ),
    ),
    FixEventKind::Stopped { limit } => (
      (Icon::TriangleAlert, PaletteColor::Warning),
      ctx.t_args(
        "ci.event.autoStopped",
        [
          ("count", limit.get().to_string()),
          ("limit", limit.get().to_string()),
          ("number", event.number.to_string()),
        ],
      ),
    ),
  };
  let open = link(ctx, id, &ctx.t("ci.openChecks"), checks_page);
  event_line(id, glyph, &text, Some(open))
}

/// How CI stands now: a line, or the card of a failure.
fn now(ctx: &mut Ctx, (share, detail, run): (&CiShare, &TaskDetail, &RunView), id: &str, agent: &Arc<str>) -> Element {
  let shown = &share.shown;
  let number = shown.number().to_string();
  let look = shown.look(&run.ci_fixes);
  let total = shown.read().map_or(0, |read| read.checks.checks.len());
  match look {
    Look::Running => {
      let text = ctx.t_args("ci.event.running", [("number", number), ("count", total.to_string())]);
      event_line(id, (Icon::LoaderCircle, PaletteColor::TextSecondary), &text, None)
    }
    Look::Passed => {
      let passed = shown.read().map_or(0, |read| words::passed(&read.checks));
      let text = ctx.t_args(
        "ci.event.passed",
        [
          ("number", number),
          ("passed", passed.to_string()),
          ("total", total.to_string()),
        ],
      );
      event_line(id, (Icon::CircleCheck, PaletteColor::Success), &text, None)
    }
    Look::NoChecks => {
      let text = ctx.t_args("ci.event.none", [("number", number)]);
      event_line(id, (Icon::CircleDashed, PaletteColor::TextMuted), &text, None)
    }
    Look::Unknown => not_read(ctx, (share, id), &shown.found.ci),
    Look::Failed | Look::Fixing => card(ctx, (share, detail, run), id, agent),
  }
}

/// *CI not read on #48 · GitHub CLI isn't signed in* with *Check again*.
fn not_read(ctx: &mut Ctx, (share, id): (&CiShare, &str), ci: &PullRequestCi) -> Element {
  let shown = &share.shown;
  let text = ctx.t_args(
    "ci.event.notRead",
    [
      ("number", shown.number().to_string()),
      ("why", ctx.t(words::why_not_read(ci)).to_string()),
    ],
  );
  let services = Services::of(ctx);
  let again = CiActions::of(ctx).check_again(&services, (share.project.project.id(), shown.number()));
  let again = ActionButton::new(format!("{id}-check-again"), ButtonKind::Ghost, ctx.t("ci.checkAgain"))
    .icon(Icon::RefreshCw)
    .small()
    .tight()
    .on_click(again);
  event_line(
    id,
    (Icon::TriangleAlert, PaletteColor::Warning),
    &text,
    Some(again.into()),
  )
}

/// The card of a failure: its head, the failing checks with their first log line, and what can be done.
fn card(ctx: &mut Ctx, (share, detail, run): (&CiShare, &TaskDetail, &RunView), id: &str, agent: &Arc<str>) -> Element {
  let shown = &share.shown;
  let Some(read) = shown.read() else {
    return Column::new().id(id.to_owned()).into();
  };
  let again = run.ci_fixes.last_sent().is_some() && shown.look(&run.ci_fixes) == Look::Failed
    || matches!(
      answer(run, shown).map(|event| event.kind),
      Some(FixEventKind::Sent {
        by: FixedBy::Automatically { count, .. },
        ..
      }) if count > 1
    );
  let title = ctx.t_args(
    if again { "ci.card.failedAgain" } else { "ci.card.failed" },
    [("number", shown.number().to_string())],
  );
  let failing: Vec<&CiCheck> = read
    .checks
    .checks
    .iter()
    .filter(|check| is_failing(check.state))
    .collect();
  let count = ctx.t_args(
    "ci.card.count",
    [
      ("failed", failing.len().to_string()),
      ("total", read.checks.checks.len().to_string()),
    ],
  );
  let rows: Vec<Element> = failing
    .iter()
    .enumerate()
    .map(|(index, check)| checks::row(ctx, (id, index), check, Place::Conversation, true))
    .collect();
  let summary = summary_link(ctx, id, (share, detail));
  let head = Row::new()
    .id(format!("{id}-head"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::CircleX, IconSize::Base, PaletteColor::Danger))
    .child(Text::new(&title).variant(ExtraTypography::FormSection.role()).nowrap())
    .child(
      Text::new(&count)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap()
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(summary);
  let foot = foot(ctx, (share, detail, run), id, agent);
  let card = Column::new()
    .id(format!("{id}-card"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Danger))
    .rounded(RadiusSize::Lg)
    .child(head)
    .with_children(rows)
    .child(foot);
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().left(ConversationSpacing::ApprovalRow.role()))
    .child(card)
    .into()
}

/// The Fix CI that answered the failure shown now, if one did.
fn answer<'a>(run: &'a RunView, shown: &ShownCi) -> Option<&'a FixEvent> {
  let failure = failure_of(&shown.read()?.checks)?;
  run.ci_fixes.sent_for(&failure)
}

/// *Summary →*, to the task's Summary, where the card shows each tail in full.
fn summary_link(ctx: &mut Ctx, id: &str, (share, detail): (&CiShare, &TaskDetail)) -> Element {
  let page = ProjectPage::task(detail.task.key().clone(), TaskView::Summary, detail.not_shared());
  let route = Route::Project(share.project.project.id(), page);
  ActionButton::new(format!("{id}-summary"), ButtonKind::Ghost, ctx.t("ci.card.summary"))
    .after(Icon::ArrowRight)
    .small()
    .tight()
    .opens(Navigation::of(ctx).to(route))
    .into()
}

/// What the card offers: *Fix CI*, *Open checks on GitHub* and whether the switch is on; or, for a failure the switch
/// answered, that send and *Turn off for this run*.
fn foot(ctx: &mut Ctx, (share, detail, run): (&CiShare, &TaskDetail, &RunView), id: &str, agent: &Arc<str>) -> Element {
  let shown = &share.shown;
  let row = Row::new()
    .id(format!("{id}-foot"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center);
  if let Some(FixEventKind::Sent {
    by: FixedBy::Automatically { count, limit },
    ..
  }) = answer(run, shown).map(|event| event.kind)
  {
    let at = answer(run, shown).map_or_else(String::new, |event| clock(event.at));
    let sent = ctx.t_args(
      "ci.card.sentAuto",
      [
        ("count", count.to_string()),
        ("limit", limit.get().to_string()),
        ("agent", agent.to_string()),
        ("time", at),
      ],
    );
    return row
      .child(icon(Icon::Repeat, IconSize::Base, PaletteColor::TextSecondary))
      .child(
        Text::new(&sent)
          .variant(TypographyStyle::Description)
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .child(turn_off(ctx, (share, run), id))
      .into();
  }
  let checks_page: Arc<str> = format!("{}/checks", shown.found.pull_request.url()).into();
  let open = ActionButton::new(format!("{id}-open-checks"), ButtonKind::Ghost, ctx.t("ci.openChecks"))
    .icon(Icon::ExternalLink)
    .on_click(open_page(&Services::of(ctx), checks_page));
  let fix_for = FixFor {
    id,
    project: &share.project,
    task: detail.task.id(),
    run: run.id(),
    lost: share.lost,
    agent,
  };
  let (fix, _) = fix_button(ctx, &fix_for, shown);
  let auto = if run.ci_fixes.auto.on {
    ctx.t_args(
      "ci.auto.used",
      [
        ("used", run.ci_fixes.used().to_string()),
        ("limit", run.ci_fixes.auto.limit.get().to_string()),
      ],
    )
  } else {
    ctx.t("ci.auto.off")
  };
  row
    .child(fix)
    .child(open)
    .child(Row::new().flex(1.0))
    .child(
      Text::new(&auto)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .into()
}

/// *Turn off for this run*.
fn turn_off(ctx: &mut Ctx, (share, run): (&CiShare, &RunView), id: &str) -> Element {
  let actions = CiActions::of(ctx);
  let (root, run_id, auto) = (share.project.repo_root.clone(), run.id(), run.ci_fixes.auto);
  ActionButton::new(format!("{id}-turn-off"), ButtonKind::Ghost, ctx.t("ci.auto.turnOff"))
    .icon(Icon::CirclePause)
    .on_click(move || {
      actions.auto.run(AutoRequest {
        root: root.clone(),
        run: run_id,
        auto: AutoFix { on: false, ..auto },
        by: super::actions::pressed_by(),
      });
    })
    .into()
}
