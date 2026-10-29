//! The inline decision card and its answered line (`Conversation cards · redesign` `p2o1w`): the card's anatomy
//! (05 `FsSCR`), one click or one digit answers, long agent words fold at three lines (05b `L7AgD` e), ✕ folds the card
//! to *Waits for your answer* (05c `hcN1X` i), and an answered line opens to the card with the chosen answer checked
//! (05c a–b).

use std::sync::{
  Arc,
  atomic::{AtomicUsize, Ordering},
};

use lurq::{app::App, layout::text_style::FontWeight};
use orchester_app::{
  theme::{self, CardsSpacing, CardsText, Mode},
  ui::decision::{Answer, AnsweredLine, Asked, Decision, Stage},
};

use crate::harness::{Kit, assert_close, gap_under};

const WIDTH: f32 = 760.0;
const WORDS: &str = "The Pencil MCP server in this session has no Export, so I can’t write the four frame PNGs.";
const QUESTION: &str = "Design tools still missing: how should I go on?";

fn asked() -> Asked {
  Asked {
    agent: "Builder".into(),
    step: "03 Implement".into(),
    time: "15:21".into(),
  }
}

/// Which answer was pressed last, by its number from 1; 0 before any.
fn recorder() -> (
  Arc<AtomicUsize>,
  impl Fn(usize) -> Answer + Send + Sync + Clone + 'static,
) {
  let pressed = Arc::new(AtomicUsize::new(0));
  let record = pressed.clone();
  (pressed, move |number: usize| {
    let record = record.clone();
    let answer = match number {
      1 => Answer::new("I export it myself").consequence("You export the frames in Pencil."),
      2 => Answer::new("Skip the exports").consequence("Builder finishes without PNGs."),
      _ => Answer::own("Answer in my own words"),
    };
    answer.on_press(move || record.store(number, Ordering::SeqCst))
  })
}

fn waiting(words: &'static str) -> (Kit, Arc<AtomicUsize>) {
  let (pressed, answer) = recorder();
  let kit = Kit::show(move |ctx| {
    lurq::components::Column::new()
      .width(lurq::node::dimension::Dimension::Px(WIDTH))
      .child(
        Decision::new("d", words, QUESTION, asked())
          .answers((1..=3).map(&answer))
          .render(ctx),
      )
      .into()
  });
  (kit, pressed)
}

#[test]
fn decision_card_draws_its_words_question_answers_with_keys_and_meta_line() {
  let (mut kit, _) = waiting(WORDS);
  let (card, words) = (kit.bounds("d-card"), kit.bounds("d-words"));
  assert_close(words.x - card.x, 20.0, "the card's side padding");
  assert_close(words.y - card.y, 18.0, "the card's top padding");
  assert_close(
    gap_under(kit.bounds("d-words"), kit.bounds("d-question")),
    12.0,
    "words to question",
  );
  for number in 1..=3 {
    assert_eq!(kit.texts(&format!("d-answer-{number}-key")), [number.to_string()]);
  }
  let (first, second) = (kit.bounds("d-answer-1"), kit.bounds("d-answer-2"));
  assert_close(
    first.width,
    card.width - 40.0,
    "an answer with a consequence fills the card",
  );
  assert_close(gap_under(first, second), 6.0, "stacked answers");
  assert!(kit.exists("d-answer-1-consequence"));
  assert_close(kit.bounds("d-answer-3").height, 28.0, "a short answer is a 28 button");
  assert!(kit.exists("d-answer-3-icon"), "own words lead with a pencil");
  assert_close(gap_under(card, kit.bounds("d-meta")), 8.0, "card to meta line");
  assert!(
    kit
      .texts("d-meta")
      .contains(&"Builder asks · 03 Implement · 15:21".to_owned())
  );
}

#[test]
fn decision_card_answers_with_one_click_or_its_digit() {
  let (mut kit, pressed) = waiting(WORDS);
  kit.click("d-answer-2");
  assert_eq!(pressed.load(Ordering::SeqCst), 2, "one click answers");
  kit.tree.key_down("1".into(), "Digit1".into(), false, false, false);
  kit.pass();
  assert_eq!(pressed.load(Ordering::SeqCst), 1, "1 presses the first answer");
  kit.tree.key_down("3".into(), "Digit3".into(), false, true, false);
  kit.pass();
  assert_eq!(pressed.load(Ordering::SeqCst), 1, "Ctrl+3 is no answer");
  kit.tree.key_down("7".into(), "Digit7".into(), false, false, false);
  kit.pass();
  assert_eq!(
    pressed.load(Ordering::SeqCst),
    1,
    "a digit without an answer does nothing"
  );
}

#[test]
fn decision_card_folds_long_words_at_three_lines_until_shown_whole() {
  let long: &'static str = "I looked for an Export tool in every MCP server this session lists: pencil-design offers \
                            Get, Insert, Copy, Update, Replace and TakeScreenshot, but its Export is turned off in \
                            this build. The Orchester server can save files only under .tmp/, and it has no image \
                            tool. I tried twice with a restart of the session in between, and both times the tool \
                            list came back the same, so the report would name four files that do not exist.";
  let (mut kit, _) = waiting(long);
  kit.pass();
  let line = {
    let style = kit.app.theme().typography_style(CardsText::Context.role());
    style.font_size * style.line_height
  };
  assert_close(kit.bounds("d-cut").height, line * 3.0, "three lines show");
  let whole = kit.texts("d-whole");
  let said = whole.iter().find(|text| text.starts_with("Show the whole question · "));
  let lines: usize = said
    .and_then(|text| {
      text
        .trim_start_matches("Show the whole question · ")
        .strip_suffix(" lines")
    })
    .and_then(|count| count.parse().ok())
    .unwrap_or_else(|| panic!("the link counts the lines: {whole:?}"));
  assert!(lines > 3, "{lines} lines");
  kit.click("d-whole");
  assert!(!kit.exists("d-whole") && !kit.exists("d-cut"), "shown whole");
  assert_close(kit.bounds("d-words").height, line * lines as f32, "every line shows");
}

#[test]
fn decision_card_short_words_are_not_folded() {
  let (mut kit, _) = waiting(WORDS);
  kit.pass();
  assert!(!kit.exists("d-whole"), "no link for words of three lines or fewer");
}

#[test]
fn decision_card_folds_to_waits_for_your_answer_with_its_cross() {
  let (mut kit, pressed) = waiting(WORDS);
  kit.click("d-fold");
  assert!(!kit.exists("d-card") && !kit.exists("d-meta"), "the card folds");
  let line = kit.texts("d-line");
  assert!(line.contains(&"Waits for your answer".to_owned()), "{line:?}");
  assert!(line.contains(&format!("· {QUESTION} · since 15:21")), "{line:?}");
  kit.tree.key_down("1".into(), "Digit1".into(), false, false, false);
  kit.pass();
  assert_eq!(pressed.load(Ordering::SeqCst), 0, "a folded card takes no digit");
  kit.click("d-line");
  assert!(kit.exists("d-card"), "the line unfolds it");
}

#[test]
fn answered_line_opens_the_card_with_the_chosen_answer_checked() {
  let (_, answer) = recorder();
  let mut kit = Kit::show(move |ctx| {
    let line = AnsweredLine::you(ctx, "“I export it myself”", "· Design tools still missing · 15:24");
    lurq::components::Column::new()
      .width(lurq::node::dimension::Dimension::Px(WIDTH))
      .child(
        Decision::new("d", WORDS, QUESTION, asked())
          .answers((1..=3).map(&answer))
          .stage(Stage::Answered {
            line,
            chosen: Some(0),
            footer: Some("Asked by Builder at 15:21 · answered by you at 15:24".into()),
          })
          .render(ctx),
      )
      .into()
  });
  assert!(!kit.exists("d-card"), "answered, the card is one line");
  let line = kit.texts("d-line");
  for said in [
    "You answered",
    "“I export it myself”",
    "· Design tools still missing · 15:24",
  ] {
    assert!(line.contains(&said.to_owned()), "{said}: {line:?}");
  }
  kit.click("d-line");
  assert!(
    kit.exists("d-card") && kit.exists("d-footer"),
    "the line opens the card"
  );
  assert!(kit.exists("d-answer-1-icon"), "the chosen answer is checked");
  assert!(!kit.exists("d-answer-2-icon"), "the others are not");
  assert!(!kit.exists("d-answer-1-key"), "an answered card shows no keys");
  assert!(!kit.exists("d-answer-1-consequence"));
  assert!(!kit.exists("d-answer-3"), "own words are left out once answered");
  assert!(!kit.exists("d-fold") && !kit.exists("d-meta"));
  kit.click("d-line");
  assert!(!kit.exists("d-card"), "a second press folds it again");
}

#[test]
fn decision_card_theme_roles_follow_the_design() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let theme = app.theme();
  for (role, (size, line_height), weight) in [
    (CardsText::Context, (13.0, 1.55), FontWeight::Normal),
    (CardsText::Decision, (15.0, 1.4), FontWeight::SemiBold),
    (CardsText::Whole, (12.0, 1.3), FontWeight::Medium),
    (CardsText::Quiet, (12.0, 1.3), FontWeight::Normal),
    (CardsText::Lead, (12.0, 1.3), FontWeight::Normal),
    (CardsText::Chosen, (12.0, 1.3), FontWeight::Medium),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.line_height, style.weight),
      (size, line_height, weight),
      "{role:?}"
    );
  }
  let palette = theme.palette();
  assert_eq!(
    theme.typography_style(CardsText::Context.role()).color,
    palette.text_secondary
  );
  assert_eq!(
    theme.typography_style(CardsText::Quiet.role()).color,
    palette.text_muted
  );
  assert_eq!(
    theme.spacing_value(CardsSpacing::InlineEnds.role()),
    lurq::node::dimension::Dimension::Px(18.0)
  );
  assert_eq!(
    theme.spacing_value(CardsSpacing::InlineSides.role()),
    lurq::node::dimension::Dimension::Px(20.0)
  );
}
