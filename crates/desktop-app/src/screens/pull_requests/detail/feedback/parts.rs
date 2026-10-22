//! The parts a feedback thread is drawn with, in the diff (`Thread · …` `nkpzr`) and beside the list (`Thread panel`
//! `Z3JKS`): its state (`State status`), that it stays in Orchester (`Feedback visibility` `RarYu`), a person's and
//! the agent's messages (`Message` `J7I7x`), what happened to the request (`Feedback event` `y1KEgI`), the agent's
//! work while it works (`Activity` `N27Ynd`), the commit it made (`Feedback commit` `vxEnt`) and the notes under
//! them (`Compare note` `VFNN5`, `Notes` `v6q9yC`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{pull_request::feedback::Located, tokens::compact_tokens};
use orchester_workspace::{
  display::short_revision,
  pull_requests::feedback::{BoardTask, CommitView, ThreadState, ThreadView},
};

use super::words;
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize, PullRequestsSpacing, PullRequestsText},
  ui::{
    avatar::{initials, person_avatar},
    button::ActionButton,
    icon::{Icon, icon},
  },
};

/// An event's icon slot (`Icon box` `D0lThX`): as wide as its icon and as high as the event's line.
const EVENT_ICON_WIDTH: f32 = 14.0;
const EVENT_ICON_HEIGHT: f32 = 18.0;

/// A thread's state: its icon and word, 5 apart, in its colors (`State status`).
pub fn status(ctx: &Ctx, id: &str, thread: &ThreadView) -> Element {
  let look = words::look(ctx, thread);
  Row::new()
    .id(id.to_owned())
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .describe("status", look.word.clone())
    .child(icon(look.icon, IconSize::Base, look.color.clone()))
    .child(
      Text::new(&look.word)
        .id(format!("{id}-label"))
        .variant(TypographyStyle::Label)
        .color(words::word_color(&look))
        .nowrap(),
    )
    .into()
}

/// *Stays in Orchester · never posted to GitHub*, or *Orchester only* where room is short (`RarYu`).
pub fn visibility(ctx: &Ctx, id: &str, short: bool) -> Row {
  let key = if short {
    "pull_requests.feedback.visibilityShort"
  } else {
    "pull_requests.feedback.visibility"
  };
  Row::new()
    .id(id.to_owned())
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::EyeOff, IconSize::Sm, PaletteColor::TextMuted))
    .child(Text::new(&ctx.t(key)).variant(TypographyStyle::Caption).nowrap())
}

/// A message's author, when and the words under them past the avatar (`Message` `J7I7x`), with more under the
/// words (the places an answer points to, a commit).
fn message(id: &str, (avatar, name, meta): (&str, &str, &str), body: &str, more: Vec<Element>) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(person_avatar(&format!("{id}-avatar"), avatar))
        .child(Text::new(name).variant(TypographyStyle::Label).nowrap())
        .child(Text::new(meta).variant(TypographyStyle::Caption).nowrap()),
    )
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::new().left(PullRequestsSpacing::FeedbackIndent.role()))
        .spacing(SpacingSize::Sm)
        .child(
          Text::new(body)
            .id(format!("{id}-body"))
            .variant(TypographyStyle::Description)
            .width(Dimension::Pct(100.0)),
        )
        .with_children(more),
    )
    .into()
}

/// The person's posts and the agent's answers, oldest first; the places the latest answer points to and the commit
/// it made under it. The person's avatar holds the initial of *You*: their name is not known here.
pub fn messages(ctx: &Ctx, thread: &ThreadView, task: &str) -> Vec<Element> {
  let you = ctx.t("pull_requests.feedback.you");
  let your_initials = initials(&you);
  let agent = &*thread.recipient;
  let agent_initials = initials(agent);
  let last = thread.posts.iter().rposition(|post| !post.withdrawn);
  let mut shown = Vec::new();
  for (at, post) in thread.posts.iter().enumerate().filter(|(_, post)| !post.withdrawn) {
    let id = format!("feedback-{}-post-{at}", thread.id);
    shown.push(message(
      &id,
      (&your_initials, &you, &words::time(post.at)),
      &post.body,
      Vec::new(),
    ));
    if let Some(reply) = &post.reply {
      let mut more: Vec<Element> = Vec::new();
      if Some(at) == last {
        more.extend(sources(ctx, thread));
        more.extend(
          thread
            .commit
            .as_ref()
            .map(|commit| commit_card(ctx, &format!("{id}-commit"), commit)),
        );
      }
      let meta = ctx.t_args(
        "pull_requests.feedback.agentMeta",
        [("task", task.to_owned()), ("time", words::time(reply.at))],
      );
      shown.push(message(
        &format!("{id}-reply"),
        (&agent_initials, agent, &meta),
        &reply.body,
        more,
      ));
    }
  }
  shown
}

/// The places the agent's answer points to, each a chip (`Sources` `owrXp`).
fn sources(ctx: &Ctx, thread: &ThreadView) -> Option<Element> {
  if thread.sources.is_empty() {
    return None;
  }
  let chips = thread.sources.iter().enumerate().map(|(at, source)| {
    let file = source.path.rsplit('/').next().unwrap_or(&source.path);
    Row::new()
      .id(format!("feedback-{}-source-{at}", thread.id))
      .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Fine.role()))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Sm)
      .child(icon(Icon::FileCode, IconSize::Sm, PaletteColor::TextMuted))
      .child(
        Text::new(&ctx.t_args(
          "pull_requests.feedback.sourceAt",
          [("file", file.to_owned()), ("line", source.line.to_string())],
        ))
        .variant(ExtraTypography::MonoKey.role())
        .color(PaletteColor::TextSecondary)
        .nowrap(),
      )
      .child(icon(Icon::ArrowUpRight, IconSize::Xs, PaletteColor::TextMuted))
      .into()
  });
  Some(
    Row::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .with_children(chips.collect::<Vec<Element>>())
      .into(),
  )
}

/// The commit the agent made for a fix: its revision and subject, what it changed and the pull request's checks
/// when GitHub has it (`Feedback commit` `vxEnt`).
pub fn commit_card(ctx: &Ctx, id: &str, commit: &CommitView) -> Element {
  let changes = commit.changes.map(|(files, additions, deletions)| {
    let args = [
      ("files", files.to_string()),
      ("additions", additions.to_string()),
      ("deletions", deletions.to_string()),
    ];
    if files == 1 {
      ctx.t_args("pull_requests.feedback.commit.changes.one", args)
    } else {
      ctx.t_args("pull_requests.feedback.commit.changes.other", args)
    }
  });
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      ExtraSpacing::Loose.role(),
      ExtraSpacing::TextArea.role(),
    ))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(icon(
      Icon::GitCommitHorizontal,
      IconSize::Base,
      PaletteColor::TextSecondary,
    ))
    .child(
      Text::new(short_revision(commit.revision.as_str()))
        .variant(ExtraTypography::MonoKey.role())
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    )
    .with_children(commit.subject.as_ref().map(|subject| {
      Text::new(subject)
        .variant(TypographyStyle::Description)
        .nowrap()
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0))
    }))
    .child(Row::new().flex(1.0))
    .with_children(changes.map(|changes| Text::new(&changes).variant(ExtraTypography::MonoKey.role()).nowrap()))
    .into()
}

/// What happened to the current request, oldest first (`Events` `EJWbi`).
pub fn events(ctx: &Ctx, thread: &ThreadView, (task, board): (&str, &[ThreadView])) -> Option<Element> {
  if thread.events.is_empty() {
    return None;
  }
  let rows = thread.events.iter().enumerate().map(|(at, event)| {
    let (glyph, word, detail, when) = words::event(ctx, event, thread, (task, board));
    Row::new()
      .id(format!("feedback-{}-event-{at}", thread.id))
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Start)
      .child(
        Row::new()
          .width(Dimension::Px(EVENT_ICON_WIDTH))
          .height(Dimension::Px(EVENT_ICON_HEIGHT))
          .align_items(Alignment::Center)
          .justify(Justify::Center)
          .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted)),
      )
      .child(Text::new(&word).variant(PullRequestsText::EventLabel.role()).nowrap())
      .child(
        Text::new(&detail)
          .variant(TypographyStyle::Description)
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .child(
        Text::new(&words::time(when))
          .variant(PullRequestsText::EventTime.role())
          .nowrap(),
      )
      .into()
  });
  Some(indented(
    &format!("feedback-{}-events", thread.id),
    rows.collect(),
    ExtraSpacing::Snug.role(),
  ))
}

/// Parts under a message, past its avatar.
fn indented(id: &str, parts: Vec<Element>, spacing: SpacingSize) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().left(PullRequestsSpacing::FeedbackIndent.role()))
    .spacing(spacing)
    .with_children(parts)
    .into()
}

/// What the agent does on the request while it works: what it reports, the run and the tokens it spent so far
/// (`Activity` `N27Ynd`), with *Open conversation*.
pub fn activity(ctx: &Ctx, thread: &ThreadView, task: &BoardTask, conversation: ActionButton) -> Option<Element> {
  let working = thread.working.as_ref()?;
  let run = task.run.as_ref()?;
  let agent = &*thread.recipient;
  let now = working
    .progress
    .clone()
    .unwrap_or_else(|| ctx.t_args("pull_requests.feedback.activity.working", [("agent", agent)]));
  let line = |glyph: Icon, text: Arc<str>| {
    Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(glyph, IconSize::Sm, PaletteColor::TextMuted))
      .child(Text::new(&text).variant(ExtraTypography::Note.role()).nowrap())
  };
  let run_line = ctx.t_args(
    "pull_requests.feedback.activity.run",
    [
      ("task", task.key.to_string()),
      ("step", working.step.to_string()),
      ("run", run.number.to_string()),
      ("branch", run.branch.to_string()),
      ("agent", agent.to_owned()),
    ],
  );
  let time = ctx.t_args(
    "pull_requests.feedback.activity.time",
    [
      ("time", words::time(working.since)),
      ("elapsed", words::elapsed(ctx, working.since).to_string()),
    ],
  );
  let budget = spent_on(ctx, thread, task);
  let card = Column::new()
    .id(format!("feedback-{}-activity", thread.id))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(Icon::LoaderCircle, IconSize::Base, PaletteColor::TextSecondary))
        .child(
          Text::new(&now)
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextPrimary)
            .nowrap()
            .flex_shrink(1.0)
            .min_width(Dimension::Px(0.0)),
        )
        .child(Row::new().flex(1.0))
        .child(conversation),
    )
    .child(
      line(Icon::ListChecks, run_line)
        .width(Dimension::Pct(100.0))
        .child(Row::new().flex(1.0))
        .child(Text::new(&time).variant(ExtraTypography::Note.role()).nowrap()),
    )
    .child(line(Icon::Coins, budget));
  Some(indented(
    &format!("feedback-{}-activity-wrap", thread.id),
    vec![card.into()],
    SpacingSize::Sm,
  ))
}

/// The tokens the agent spent on `thread`'s request, and what is left of the task's budget when it has one.
fn spent_on(ctx: &Ctx, thread: &ThreadView, task: &BoardTask) -> Arc<str> {
  let tokens = compact_tokens(thread.tokens.unwrap_or_default());
  match task.run.as_ref().and_then(|run| run.budget) {
    Some(budget) => {
      let (left, limit) = words::budget(budget);
      ctx.t_args(
        "pull_requests.feedback.activity.tokens",
        [
          ("tokens", tokens),
          ("left", left),
          ("task", task.key.to_string()),
          ("budget", limit),
        ],
      )
    }
    None => ctx.t_args("pull_requests.feedback.activity.tokensNoBudget", [("tokens", tokens)]),
  }
}

/// The notes under the messages: what changed since a fix's request and what it cost, or that a question changes no
/// code, or why an outdated thread stays where it was (`Compare note` `VFNN5`, `Notes` `v6q9yC`, `Why` `VVK3T`).
pub fn notes(
  ctx: &Ctx,
  thread: &ThreadView,
  (task, head): (&BoardTask, Option<&orchester_core::revision::Revision>),
) -> Option<Element> {
  let note = |glyph: Icon, text: Arc<str>| -> Element {
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Start)
      .child(icon(glyph, IconSize::Sm, PaletteColor::TextMuted))
      .child(
        Text::new(&text)
          .variant(ExtraTypography::Note.role())
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .into()
  };
  let mut parts: Vec<Element> = Vec::new();
  if let (Some(Located::Outdated), Some(head)) = (thread.at_head, head)
    && thread.state.is_open()
  {
    parts.push(note(Icon::History, outdated(ctx, thread, head)));
  }
  match &thread.state {
    ThreadState::ReadyToVerify { commit, .. } => {
      parts.extend(compared(ctx, thread, commit).map(|text| note(Icon::GitCompare, text)));
      parts.extend(used(ctx, thread, task, "usedRun").map(|text| note(Icon::Coins, text)));
      parts.push(note(Icon::Info, ctx.t("pull_requests.feedback.notes.resolveOnly")));
    }
    ThreadState::Answered { .. } if thread.kind == orchester_core::pull_request::feedback::FeedbackKind::Question => {
      parts.push(note(Icon::Info, ctx.t("pull_requests.feedback.notes.noCode")));
      parts.extend(used(ctx, thread, task, "answerUsed").map(|text| note(Icon::Coins, text)));
    }
    _ => {}
  }
  (!parts.is_empty()).then(|| {
    indented(
      &format!("feedback-{}-notes", thread.id),
      parts,
      ExtraSpacing::Snug.role(),
    )
  })
}

/// Why an outdated thread stays where it was.
fn outdated(ctx: &Ctx, thread: &ThreadView, head: &orchester_core::revision::Revision) -> Arc<str> {
  let lines = thread
    .anchor
    .lines()
    .map(|lines| words::lines(ctx, (None, lines.span())).to_string())
    .unwrap_or_default();
  ctx.t_args(
    "pull_requests.feedback.notes.outdated",
    [
      ("lines", lines),
      ("revision", short_revision(thread.revision.as_str()).to_owned()),
      ("head", short_revision(head.as_str()).to_owned()),
      ("agent", thread.recipient.to_string()),
    ],
  )
}

/// Where a fix's lines are at its commit, compared with the request's revision.
fn compared(ctx: &Ctx, thread: &ThreadView, commit: &orchester_core::revision::Revision) -> Option<Arc<str>> {
  let lines = thread.anchor.lines()?;
  let at = words::lines(ctx, (None, lines.span())).to_string();
  let (from, to) = (
    short_revision(thread.current().revision.as_str()).to_owned(),
    short_revision(commit.as_str()).to_owned(),
  );
  Some(match thread.at_head {
    Some(Located::At(moved)) => ctx.t_args(
      "pull_requests.feedback.notes.compared",
      [
        ("from", from),
        ("to", to),
        ("lines", at),
        ("moved", words::span_text(moved)),
      ],
    ),
    _ => ctx.t_args(
      "pull_requests.feedback.notes.comparedGone",
      [("from", from), ("to", to), ("lines", at)],
    ),
  })
}

/// What the request cost the task: its run, how long and the tokens, and what is left of the budget.
fn used(ctx: &Ctx, thread: &ThreadView, task: &BoardTask, which: &str) -> Option<Arc<str>> {
  let run = task.run.as_ref()?;
  let tokens = compact_tokens(thread.tokens?);
  let elapsed = thread
    .events
    .iter()
    .find_map(|event| match event {
      orchester_workspace::pull_requests::feedback::ThreadEvent::Working { at } => Some(*at),
      _ => None,
    })
    .map(|since| words::elapsed(ctx, since).to_string())
    .unwrap_or_default();
  let key = |suffix: &str| format!("pull_requests.feedback.notes.{which}{suffix}");
  Some(match run.budget {
    Some(budget) => {
      let (left, limit) = words::budget(budget);
      ctx.t_args(
        &key(""),
        [
          ("run", run.number.to_string()),
          ("task", task.key.to_string()),
          ("elapsed", elapsed),
          ("tokens", tokens),
          ("left", left),
          ("budget", limit),
        ],
      )
    }
    None => ctx.t_args(
      &key("NoBudget"),
      [
        ("run", run.number.to_string()),
        ("task", task.key.to_string()),
        ("elapsed", elapsed),
        ("tokens", tokens),
      ],
    ),
  })
}

/// The row line of a fix ready to verify: who committed what when, and what it changed.
pub fn commit_meta(ctx: &Ctx, thread: &ThreadView) -> Arc<str> {
  let Some(commit) = &thread.commit else {
    return ctx.t("pull_requests.feedback.state.readyToVerify");
  };
  let (plain, with_changes) = if commit.on_pull_request {
    (
      "pull_requests.feedback.meta.pushed",
      "pull_requests.feedback.meta.pushedChanges",
    )
  } else {
    (
      "pull_requests.feedback.meta.committed",
      "pull_requests.feedback.meta.committedChanges",
    )
  };
  let agent = thread.recipient.to_string();
  let sha = short_revision(commit.revision.as_str()).to_owned();
  let time = words::time(commit.at);
  match commit.changes {
    Some((files, additions, deletions)) => {
      let args = [
        ("files", files.to_string()),
        ("additions", additions.to_string()),
        ("deletions", deletions.to_string()),
      ];
      let changes = if files == 1 {
        ctx.t_args("pull_requests.feedback.commit.changes.one", args)
      } else {
        ctx.t_args("pull_requests.feedback.commit.changes.other", args)
      };
      ctx.t_args(
        with_changes,
        [
          ("agent", agent),
          ("commit", sha),
          ("time", time),
          ("changes", changes.to_string()),
        ],
      )
    }
    None => ctx.t_args(plain, [("agent", agent), ("commit", sha), ("time", time)]),
  }
}
