//! The inline decision card and its answered line (`Conversation cards · redesign` `p2o1w`): a question with
//! consequences (05 `FsSCR` in `yrI3M`), short answers in one row (05b `L7AgD` a), long agent words folded (05b e),
//! and the answered lines of 05c `hcN1X` (a–b: press the first line to open it; e–h). ✕ on a waiting card folds it
//! to *Waits for your answer* (05c i). The words are the design's fixture text.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_app::ui::decision::{Answer, AnsweredLine, Asked, Decision, Mark, Stage};

use super::{line, stack, wide_cell};
use crate::gallery::State;

/// The conversation's transcript width (`W2Up3` in `yrI3M`).
const TRANSCRIPT: f32 = 760.0;

const WORDS: &str = "The Pencil MCP server in this session has no Export, so I can’t write the four frame PNGs. The \
                     frames themselves are drawn and checked.";
const QUESTION: &str = "Design tools still missing: how should I go on?";
const LONG_WORDS: &str = "I looked for an Export tool in every MCP server this session lists: pencil-design offers Get, \
                          Insert, Copy, Update, Replace and TakeScreenshot, but its Export is turned off in this \
                          build. The Orchester server can save files only under .tmp/, and it has no image tool. I \
                          tried twice with a restart of the session in between, and both times the tool list came \
                          back the same. Without Export the frames can't become the PNGs the task asks for, so the \
                          report would name four files that do not exist. I can stop here and wait, finish without \
                          them, or try once more after the session restarts its MCP servers, which costs a few \
                          minutes but sometimes brings back tools a server registered late.";

pub fn section(ctx: &mut Ctx, state: &State) -> Element {
  stack([
    line([wide_cell(
      "Question · waiting · consequences · 05 FsSCR (yrI3M)",
      TRANSCRIPT,
      exports(ctx, state),
    )])
    .into(),
    line([wide_cell(
      "Question · short options · 05b L7AgD a",
      TRANSCRIPT,
      short(ctx, state),
    )])
    .into(),
    line([wide_cell(
      "Question · long agent text folded · 05b L7AgD e",
      TRANSCRIPT,
      long(ctx, state),
    )])
    .into(),
    line([wide_cell(
      "Answered lines · 05c hcN1X a–b (press the first line), e–h",
      TRANSCRIPT,
      answered(ctx),
    )])
    .into(),
  ])
}

fn asked() -> Asked {
  Asked {
    agent: "Builder".into(),
    step: "03 Implement".into(),
    time: "15:21".into(),
  }
}

/// Says in the gallery's header which answer was pressed.
fn said(state: &State, what: &'static str) -> impl Fn() + Send + Sync + 'static {
  let pressed = state.pressed.clone();
  move || pressed.set(what.to_owned())
}

/// The three options with their consequences, then *Answer in my own words*.
fn options(state: &State) -> [Answer; 4] {
  [
    Answer::new("I export it myself")
      .consequence("You export the frames in Pencil; Builder waits for the files in .tmp/design/.")
      .on_press(said(state, "I export it myself")),
    Answer::new("Skip the exports")
      .consequence("Builder finishes without PNGs and says so in its report.")
      .on_press(said(state, "Skip the exports")),
    Answer::new("Restart the session once")
      .consequence("Builder restarts its CLI to load the MCP tools again, then retries.")
      .on_press(said(state, "Restart the session once")),
    Answer::own("Answer in my own words").on_press(said(state, "Answer in my own words")),
  ]
}

fn exports(ctx: &mut Ctx, state: &State) -> Element {
  Decision::new("g-decision-waiting", WORDS, QUESTION, asked())
    .answers(options(state))
    .on_reply(said(state, "Reply"))
    .render(ctx)
}

fn short(ctx: &mut Ctx, state: &State) -> Element {
  Decision::new(
    "g-decision-short",
    "Two of the four frames reuse the Summary header; the other two need a new one. The new header is drawn in 05b.",
    "Use the new header on all four frames?",
    asked(),
  )
  .answers([
    Answer::new("Yes, all four").on_press(said(state, "Yes, all four")),
    Answer::new("Only the two new ones").on_press(said(state, "Only the two new ones")),
    Answer::own("Answer in my own words").on_press(said(state, "Answer in my own words")),
  ])
  .on_reply(said(state, "Reply"))
  .render(ctx)
}

fn long(ctx: &mut Ctx, state: &State) -> Element {
  Decision::new("g-decision-long", LONG_WORDS, QUESTION, asked())
    .answers([
      Answer::new("I export it myself").on_press(said(state, "I export it myself")),
      Answer::new("Skip the exports").on_press(said(state, "Skip the exports")),
      Answer::new("Restart once").on_press(said(state, "Restart once")),
      Answer::own("Answer in my own words").on_press(said(state, "Answer in my own words")),
    ])
    .on_reply(said(state, "Reply"))
    .render(ctx)
}

/// The card answered with its first option, then the lines of the other ways a question ends.
fn answered(ctx: &mut Ctx) -> Column {
  let you = AnsweredLine::you(ctx, "“I export it myself”", "· Design tools still missing · 15:24");
  let chosen = Decision::new("g-decision-answered", WORDS, QUESTION, asked())
    .answers(options_without_presses())
    .stage(Stage::Answered {
      line: you,
      chosen: Some(0),
      footer: Some("Asked by Builder at 15:21 · answered by you at 15:24 · Builder resumed with it".into()),
    })
    .render(ctx);
  let ended = |ctx: &mut Ctx, id: &str, line: AnsweredLine| {
    Decision::new(id, WORDS, QUESTION, asked())
      .answers(options_without_presses())
      .stage(Stage::Answered {
        line,
        chosen: None,
        footer: None,
      })
      .render(ctx)
  };
  let orchestrator = AnsweredLine::orchestrator(ctx, "“Skip the exports”", "· you let it answer questions · 15:24");
  let automation = AnsweredLine::automation(
    ctx,
    "“Restart the session once”",
    "· through Orchester’s MCP server · 15:24",
  );
  let run_ended = AnsweredLine::not_answered(
    ctx,
    Mark::Ended,
    "· Design tools still missing · the run ended at 16:02",
  );
  let stopped = AnsweredLine::not_answered(
    ctx,
    Mark::Stopped,
    "· Builder stopped waiting at 15:51, when the run was interrupted",
  );
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .child(chosen)
    .child(ended(ctx, "g-decision-orchestrator", orchestrator))
    .child(ended(ctx, "g-decision-automation", automation))
    .child(ended(ctx, "g-decision-ended", run_ended))
    .child(ended(ctx, "g-decision-stopped", stopped))
}

/// [`options`] as an answered card keeps them: no answer takes a press any more.
fn options_without_presses() -> [Answer; 4] {
  [
    Answer::new("I export it myself")
      .consequence("You export the frames in Pencil; Builder waits for the files in .tmp/design/."),
    Answer::new("Skip the exports").consequence("Builder finishes without PNGs and says so in its report."),
    Answer::new("Restart the session once")
      .consequence("Builder restarts its CLI to load the MCP tools again, then retries."),
    Answer::own("Answer in my own words"),
  ]
}
