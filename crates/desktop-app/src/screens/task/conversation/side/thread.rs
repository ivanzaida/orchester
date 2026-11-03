//! The pane's thread (`Thread` `qCxSk` in 12b): each question as the person's bubble, 300 wide at the end, with *Asked
//! {time} · only you see it* under it; each answer with the side agent's avatar, *Side question* with its model and
//! time, its text, the sources it relies on as 30-high rows (a changed file opens in Changes), and its actions:
//! *Send to {agent}* (unavailable once nobody would read it), or *Sent to {agent} · {time}* once sent, *Copy*, and
//! *Not sent*. While an answer comes, *Reading run {n}'s transcript and changes…*; one that failed says why, with
//! *Retry*; a run with no transcript yet says so (12d).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::SideQuestionId,
  run::{
    Answerer,
    side_question::{SideQuestion, SideReply, SideSource, SideState, SourceKind},
  },
  time::Timestamp,
};
use orchester_workspace::{display::clock, tasks::side_question::failure_message};

use super::{Copied, Pending, SideQuestions, pane::PaneProps};
use crate::{
  navigation::Navigation,
  routes::{ChangeSource, ChangedFile, ProjectPage, Route, TaskView},
  theme::{ControlHeight, ExtraSpacing, IconSize, SideSpacing, SideText},
  ui::{
    avatar::agent_avatar,
    button::{ActionButton, ButtonKind},
    clipboard::Clipboard,
    icon::{Icon, icon},
    message::message_text,
    pressable::opening,
  },
};

/// The question's bubble (`LXHlU`).
const BUBBLE_WIDTH: f32 = 300.0;

/// The thread of the run's side questions; `readable`: the run has a transcript to read.
pub(super) fn thread(
  ctx: &mut Ctx,
  (props, state): (&PaneProps, &SideQuestions),
  questions: &[SideQuestion],
  readable: bool,
) -> Element {
  let pending = state
    .pending
    .get()
    .filter(|pending| pending.run == props.run && !questions.iter().any(|question| question.id == pending.id));
  let mut rows: Vec<Element> = Vec::new();
  for question in questions {
    rows.push(asked(
      ctx,
      &question.id.to_string(),
      question.question.as_str(),
      question.asked_at,
    ));
    rows.push(answer(ctx, (props, state), question));
  }
  if let Some(Pending { id, text, at, .. }) = &pending {
    rows.push(asked(ctx, &id.to_string(), text, *at));
    rows.push(answering(ctx, props, &id.to_string()));
  }
  if rows.is_empty() && !readable {
    rows.push(nothing_to_read(ctx, props));
  }
  Column::new()
    .id("side-question-list")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(ExtraSpacing::Inset.role())
    .with_children(rows)
    .into()
}

/// The person's question, `text`, asked at `at` (`CYv2w`).
fn asked(ctx: &mut Ctx, id: &str, text: &str, at: Timestamp) -> Element {
  let meta = ctx.t_args("side_question.asked", [("time", clock(at).as_str())]);
  Column::new()
    .id(format!("side-question-asked-{id}"))
    .width(Dimension::Pct(100.0))
    .spacing(SideSpacing::Edge.role())
    .align_items(Alignment::End)
    .child(
      Row::new()
        .width(Dimension::Px(BUBBLE_WIDTH))
        .padding(
          Padding::new()
            .top(ExtraSpacing::Loose.role())
            .bottom(ExtraSpacing::Loose.role())
            .left(ExtraSpacing::Inset.role())
            .right(ExtraSpacing::Inset.role()),
        )
        .background(PaletteColor::SurfaceRaised)
        .rounded(RadiusSize::Lg)
        .child(
          Text::new(text)
            .id(format!("side-question-text-{id}"))
            .variant(SideText::Body.role())
            .flex(1.0)
            .min_width(Dimension::Px(0.0)),
        ),
    )
    .child(Text::new(&meta).variant(SideText::Meta.role()).nowrap())
    .into()
}

/// The answer's row: the avatar beside `content` under its head, which says `meta` (`rVqTf`).
fn answer_frame(ctx: &mut Ctx, id: &str, meta: &str, content: Vec<Element>) -> Element {
  let head = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("side_question.title"))
        .variant(SideText::Who.role())
        .nowrap(),
    )
    .child(
      Text::new(meta)
        .id(format!("side-answer-meta-{id}"))
        .variant(SideText::Quiet.role())
        .nowrap(),
    );
  Row::new()
    .id(format!("side-answer-{id}"))
    .width(Dimension::Pct(100.0))
    .spacing(SideSpacing::Gap.role())
    .align_items(Alignment::Start)
    .child(agent_avatar(
      &format!("side-answer-avatar-{id}"),
      Icon::MessageCircleQuestion,
    ))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(SpacingSize::Sm)
        .child(head)
        .with_children(content),
    )
    .into()
}

/// The side agent answering: *Reading run {n}'s transcript and changes…* with the turning loader.
fn answering(ctx: &mut Ctx, props: &PaneProps, id: &str) -> Element {
  let reading = ctx.t_args("side_question.reading", [("run", props.number.to_string().as_str())]);
  let line = Row::new()
    .id(format!("side-answer-reading-{id}"))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::LoaderCircle, IconSize::Md, PaletteColor::TextMuted))
    .child(Text::new(&reading).variant(SideText::Scope.role()))
    .into();
  let meta = ctx.t("side_question.answering");
  answer_frame(ctx, id, &meta, vec![line])
}

/// How `question` stands: being answered, answered, or not.
fn answer(ctx: &mut Ctx, (props, state): (&PaneProps, &SideQuestions), question: &SideQuestion) -> Element {
  let id = question.id.to_string();
  match &question.state {
    SideState::Answering => answering(ctx, props, &id),
    SideState::Answered { reply, model, at, sent } => {
      let meta = ctx.t_args(
        "side_question.model",
        [("model", model.as_str()), ("time", &clock(*at))],
      );
      let mut content = vec![
        Text::new(&reply.text)
          .id(format!("side-answer-text-{id}"))
          .variant(SideText::Body.role())
          .width(Dimension::Pct(100.0))
          .into(),
      ];
      content.extend(sources(ctx, props, &id, &reply.sources));
      content.push(actions(ctx, (props, state), (question.id, reply, *sent)));
      answer_frame(ctx, &id, &meta, content)
    }
    SideState::Failed { failure, .. } => {
      let why = message_text(ctx, &failure_message(failure));
      let (state, project, task, run) = (state.clone(), props.project.open.clone(), props.task, props.run);
      let text = question.question.as_str().to_owned();
      let retry = ActionButton::new(
        format!("side-answer-retry-{id}"),
        ButtonKind::Ghost,
        ctx.t("side_question.retry"),
      )
      .icon(Icon::RotateCcw)
      .on_click(move || state.ask(&project, (task, run), &text));
      let failed = Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(Icon::CircleAlert, IconSize::Md, PaletteColor::Danger))
        .child(
          Text::new(&why)
            .id(format!("side-answer-failed-{id}"))
            .variant(SideText::Failed.role())
            .flex(1.0)
            .min_width(Dimension::Px(0.0)),
        )
        .child(retry)
        .into();
      let meta = String::new();
      answer_frame(ctx, &id, &meta, vec![failed])
    }
  }
}

/// The sources the answer relies on (`ur1WE`); none when it named none.
fn sources(ctx: &mut Ctx, props: &PaneProps, id: &str, sources: &[SideSource]) -> Option<Element> {
  if sources.is_empty() {
    return None;
  }
  let rows: Vec<Element> = sources
    .iter()
    .enumerate()
    .map(|(at, source)| source_row(ctx, props, &format!("{id}-{at}"), source))
    .collect();
  Some(
    Column::new()
      .id(format!("side-answer-sources-{id}"))
      .width(Dimension::Pct(100.0))
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .with_children(rows)
      .into(),
  )
}

/// One source: its kind's icon, its name (mono for a path or a command), what it is, and for a changed file *Open ↗*
/// to it in Changes.
fn source_row(ctx: &mut Ctx, props: &PaneProps, id: &str, source: &SideSource) -> Element {
  let (glyph, name) = match source.kind {
    SourceKind::File => (Icon::FileText, SideText::SourceMono),
    SourceKind::Command => (Icon::Terminal, SideText::SourceMono),
    SourceKind::Message => (Icon::MessageSquare, SideText::Source),
  };
  // `.ok()`: a source with an empty path names no file, and opens nothing.
  let open = (source.kind == SourceKind::File)
    .then(|| ChangedFile::new(source.name.clone(), ChangeSource::default()).ok())
    .flatten()
    .map(|file| {
      let route = Route::Project(
        props.project.open.project.id(),
        ProjectPage::task(props.key.clone(), TaskView::Changes(Some(file)), props.not_shared),
      );
      let label = ctx.t("side_question.open");
      opening(Navigation::of(ctx).to(route))
        .id(format!("side-source-open-{id}"))
        .spacing(ExtraSpacing::Fine.role())
        .child(Text::new(&label).variant(SideText::Open.role()).nowrap())
        .child(icon(Icon::ArrowUpRight, IconSize::Sm, PaletteColor::TextSecondary))
    });
  Row::new()
    .id(format!("side-source-{id}"))
    .describe("kind", format!("{:?}", source.kind))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ControlHeight::Row.value()))
    .padding(Padding::horizontal(SideSpacing::Edge.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph, IconSize::Sm, PaletteColor::TextMuted))
    .child(Text::new(&source.name).variant(name.role()).nowrap())
    .child(
      Text::new(&source.detail)
        .variant(SideText::Detail.role())
        .nowrap()
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(open)
    .into()
}

/// *Send to {agent}* or *Sent to {agent} · {time}*, *Copy*, and *Not sent* while it was not (`U6rRBx`).
fn actions(
  ctx: &mut Ctx,
  (props, state): (&PaneProps, &SideQuestions),
  (question, reply, sent): (SideQuestionId, &SideReply, Option<Timestamp>),
) -> Element {
  let id = question.to_string();
  let send: Element = match sent {
    Some(at) => {
      let words = ctx.t_args("side_question.sent", [("agent", &*props.agent), ("time", &clock(at))]);
      Row::new()
        .id(format!("side-answer-sent-{id}"))
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::CheckCheck, IconSize::Md, PaletteColor::Success))
        .child(Text::new(&words).variant(SideText::Quiet.role()).nowrap())
        .into()
    }
    None => {
      let label = ctx.t_args("side_question.send", [("agent", &*props.agent)]);
      let button = ActionButton::new(format!("side-answer-send-{id}"), ButtonKind::Secondary, label).icon(Icon::Send);
      if !props.sendable {
        let why = ctx.t_args("side_question.runEnded", [("run", props.number.to_string().as_str())]);
        button.unavailable(why).into()
      } else if state.sending() {
        button.loading().into()
      } else {
        let (state, project, task) = (state.clone(), props.project.open.clone(), props.task);
        button
          .on_click(move || {
            // Who pressed is read while the press is delivered, as the run composer reads it.
            let by = if lurq::app::events::is_synthetic_input() {
              Answerer::Automation
            } else {
              Answerer::Person
            };
            state.send(&project, (task, question), by);
          })
          .into()
      }
    }
  };
  let copied = state.copied.get() == Copied(Some(question));
  let copy_label = ctx.t(if copied {
    "side_question.copied"
  } else {
    "side_question.copy"
  });
  let (clipboard, text, marks) = (Clipboard::of(ctx), reply.text.clone(), state.copied.clone());
  let copy = ActionButton::new(format!("side-answer-copy-{id}"), ButtonKind::Ghost, copy_label)
    .icon(Icon::Copy)
    .on_click(move || {
      if clipboard.copy(&text) {
        marks.set(Copied(Some(question)));
      }
    });
  let not_sent = sent.is_none().then(|| {
    Text::new(&ctx.t("side_question.notSent"))
      .id(format!("side-answer-not-sent-{id}"))
      .variant(SideText::Quiet.role())
      .nowrap()
  });
  Row::new()
    .id(format!("side-answer-actions-{id}"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SideSpacing::Edge.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(send)
    .child(copy)
    .child(Spacer::new().flex(1.0))
    .with_children(not_sent)
    .into()
}

/// A run with no transcript yet: nothing to read (12d).
fn nothing_to_read(ctx: &mut Ctx, props: &PaneProps) -> Element {
  let words = ctx.t_args(
    "side_question.nothingToRead",
    [("run", props.number.to_string().as_str()), ("agent", &*props.agent)],
  );
  Row::new()
    .id("side-question-nothing")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::CircleHelp, IconSize::Md, PaletteColor::TextMuted))
    .child(
      Text::new(&words)
        .variant(SideText::Scope.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}
