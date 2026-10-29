//! The card's view: waiting (05 `FsSCR`, 05b `L7AgD` a and e), folded with ✕ (05c `hcN1X` i), and answered — its
//! line, opened to the card as it was (05c a–b).

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{Answer, AnsweredLine, Decision, Handler, Mark, Stage};
use crate::{
  theme::{CardsSpacing, CardsText, ExtraSpacing, IconSize, NOT_CHOSEN_OPACITY},
  ui::{
    button::{ActionButton, ButtonKind},
    clipboard::Clipboard,
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// The agent's words show this many lines before *Show the whole question* (`DG9gP` in 05b `L7AgD` e).
const FOLD_LINES: usize = 3;

/// How the answers are drawn.
#[derive(Clone, Copy)]
enum Answers {
  /// They wait: each takes a press and its key.
  Open,
  /// The card was answered with the one at this index, or none: they no longer take a press.
  Given(Option<usize>),
}

pub(super) struct DecisionView {
  /// A waiting card folded with ✕.
  folded: Signal<bool>,
  /// An answered card's line opened to the card.
  open: Signal<bool>,
  /// The agent's words shown whole.
  whole: Signal<bool>,
  /// How many lines the agent's words take at the card's width, as laid out.
  lines: Signal<usize>,
  words: ElementRef,
}

impl Component for DecisionView {
  type Props = Decision;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      folded: ctx.signal(false),
      open: ctx.signal(false),
      whole: ctx.signal(false),
      lines: ctx.signal(0),
      words: ElementRef::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let decision = ctx.props::<Decision>().clone();
    match &decision.stage {
      Stage::Waiting if self.folded.get() => self.folded(ctx, &decision),
      Stage::Waiting => self.waiting(ctx, &decision),
      Stage::Answered { line, chosen, footer } => self.answered(ctx, &decision, line, (*chosen, footer.as_ref())),
    }
  }
}

impl DecisionView {
  /// The card, the meta line under it; the digits press the answers.
  fn waiting(&self, ctx: &mut Ctx, decision: &Decision) -> Element {
    let id = &decision.id;
    let folded = self.folded.clone();
    let fold = pressable(move || folded.set(true))
      .id(format!("{id}-fold"))
      .describe("a11y_name", ctx.t("cards.fold"))
      .child(icon(Icon::X, IconSize::Lg, PaletteColor::TextMuted));
    let top = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .align_items(Alignment::Start)
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(SpacingSize::Md)
          .with_children(self.words(ctx, decision))
          .child(question(decision)),
      )
      .child(fold);
    let card = frame(id)
      .spacing(SpacingSize::Lg)
      .child(top)
      .with_children(answers(decision, Answers::Open));
    let presses: Vec<Option<Handler>> = decision.answers.iter().map(|answer| answer.press.clone()).collect();
    Column::new()
      .id(id.to_string())
      .describe("stage", "waiting")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .on_key_down(move |event: KeyboardEvent| press_digit(&presses, &event))
      .child(card)
      .child(meta(ctx, decision))
      .into()
  }

  /// The waiting card folded with ✕ into one line, which a press unfolds.
  fn folded(&self, ctx: &Ctx, decision: &Decision) -> Element {
    let detail = ctx.t_args(
      "cards.foldedDetail",
      [("question", &*decision.question), ("time", &*decision.asked.time)],
    );
    let unfold = self.folded.clone();
    let line = AnsweredLine::new(Mark::Waiting, ctx.t("cards.folded"), None, detail).render(
      &format!("{}-line", decision.id),
      Icon::ChevronDown,
      move || unfold.set(false),
    );
    Column::new()
      .id(decision.id.to_string())
      .describe("stage", "folded")
      .width(Dimension::Pct(100.0))
      .child(line)
      .into()
  }

  /// The answered line; opened, the card as it was under it.
  fn answered(
    &self,
    ctx: &mut Ctx,
    decision: &Decision,
    line: &AnsweredLine,
    (chosen, footer): (Option<usize>, Option<&Arc<str>>),
  ) -> Element {
    let id = &decision.id;
    let open = self.open.get();
    let toggle = self.open.clone();
    let chevron = if open { Icon::ChevronDown } else { Icon::ChevronRight };
    let line = line.render(&format!("{id}-line"), chevron, move || {
      toggle.set(!toggle.get_untracked())
    });
    let card = open.then(|| {
      frame(id)
        .spacing(SpacingSize::Md)
        .with_children(self.words(ctx, decision))
        .child(question(decision))
        .with_children(answers(decision, Answers::Given(chosen)))
        .with_children(footer.map(|footer| {
          Text::new(footer)
            .id(format!("{id}-footer"))
            .variant(CardsText::Quiet.role())
            .width(Dimension::Pct(100.0))
        }))
    });
    Column::new()
      .id(id.to_string())
      .describe("stage", "answered")
      .describe("open", open.to_string())
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .child(line)
      .with_children(card)
      .into()
  }

  /// The agent's words, cut at [`FOLD_LINES`] with *Show the whole question · N lines* under them until shown
  /// whole; nothing when the agent wrote none. The lines are counted from the words' laid-out height.
  fn words(&self, ctx: &mut Ctx, decision: &Decision) -> Vec<Element> {
    if decision.words.trim().is_empty() {
      return Vec::new();
    }
    let id = &decision.id;
    let line = CardsText::Context.line_height(ctx.theme());
    let lines = self.lines.clone();
    self.words.observe_rect(move |rect| {
      let counted = laid_out_lines(rect.height, line);
      if counted > 0 && counted != lines.get_untracked() {
        lines.set(counted);
      }
    });
    let text = Text::new(&decision.words)
      .id(format!("{id}-words"))
      .ref_element(self.words.clone())
      .variant(CardsText::Context.role())
      .width(Dimension::Pct(100.0))
      .flex_shrink(0.0);
    if self.whole.get() {
      return vec![text.into()];
    }
    // Cut from the first layout on, so long words never show whole for a frame; the column clips what passes it,
    // and the text keeps its whole height inside, which counts its lines.
    let cut = Column::new()
      .id(format!("{id}-cut"))
      .width(Dimension::Pct(100.0))
      .max_height(Dimension::Px(line * FOLD_LINES as f32))
      .child(text);
    let lines = self.lines.get();
    if lines <= FOLD_LINES {
      return vec![cut.into()];
    }
    let whole = self.whole.clone();
    let show = pressable(move || whole.set(true))
      .id(format!("{id}-whole"))
      .spacing(SpacingSize::Sm)
      .child(icon(Icon::ChevronDown, IconSize::Base, PaletteColor::TextSecondary))
      .child(
        Text::new(&ctx.t_args("cards.question.whole", [("count", lines.to_string().as_str())]))
          .variant(CardsText::Whole.role())
          .nowrap(),
      );
    vec![cut.into(), show.into()]
  }
}

/// How many lines of `line` height a text `height` high takes.
fn laid_out_lines(height: f32, line: f32) -> usize {
  if line <= 0.0 || height <= 0.0 {
    return 0;
  }
  // A text's height is a whole number of its lines; rounding absorbs the layout's fractions.
  (height / line).round() as usize
}

/// The card's outline: no fill, a `border` stroke, radius 10, 18 over and under and 20 at the sides.
fn frame(id: &str) -> Column {
  Column::new()
    .id(format!("{id}-card"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      CardsSpacing::InlineSides.role(),
      CardsSpacing::InlineEnds.role(),
    ))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
}

fn question(decision: &Decision) -> Text {
  Text::new(&decision.question)
    .id(format!("{}-question", decision.id))
    .variant(CardsText::Decision.role())
    .width(Dimension::Pct(100.0))
}

/// The answers with a consequence, stacked at full width 6 apart, then the others in one row 8 apart. Waiting, each
/// takes a press and shows its key; given, the chosen one is checked, the others dimmed, and the person's own words
/// are left out.
fn answers(decision: &Decision, state: Answers) -> Vec<Element> {
  let shown: Vec<(usize, &Answer)> = decision
    .answers
    .iter()
    .enumerate()
    .filter(|(_, answer)| matches!(state, Answers::Open) || !answer.own)
    .collect();
  let button = |(index, answer): &(usize, &Answer)| answer_button(&decision.id, (*index, answer), state);
  let stacked: Vec<Element> = shown
    .iter()
    .filter(|(_, answer)| answer.consequence.is_some())
    .map(button)
    .collect();
  let row: Vec<Element> = shown
    .iter()
    .filter(|(_, answer)| answer.consequence.is_none())
    .map(button)
    .collect();
  let mut blocks = Vec::new();
  if !stacked.is_empty() {
    blocks.push(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .with_children(stacked)
        .into(),
    );
  }
  if !row.is_empty() {
    blocks.push(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .with_children(row)
        .into(),
    );
  }
  blocks
}

/// The answer at `index`: the first is primary, the others secondary.
fn answer_button(id: &str, (index, answer): (usize, &Answer), state: Answers) -> Element {
  let kind = if index == 0 {
    ButtonKind::Primary
  } else {
    ButtonKind::Secondary
  };
  let button = ActionButton::new(format!("{id}-answer-{}", index + 1), kind, answer.label.clone());
  let button = if answer.own {
    button.icon(Icon::PencilLine)
  } else {
    button
  };
  match state {
    Answers::Open => {
      let button = button.key((index + 1).to_string()).in_form();
      let button = match &answer.consequence {
        Some(words) => button.consequence(words.clone()),
        None => button,
      };
      match &answer.press {
        Some(press) => {
          let press = press.clone();
          button.on_click(move || press()).into()
        }
        None => button.into(),
      }
    }
    Answers::Given(chosen) => {
      let stacked = answer.consequence.is_some();
      let button = if stacked { button.stacked() } else { button };
      if chosen == Some(index) {
        return button.icon(Icon::Check).shown().into();
      }
      let dimmed = Column::new()
        .opacity(NOT_CHOSEN_OPACITY)
        .describe("dimmed", "true")
        .child(button.shown());
      // A stacked answer is as wide as its parent, so its wrapper is too; a row answer keeps its own width.
      let dimmed = if stacked {
        dimmed.width(Dimension::Pct(100.0))
      } else {
        dimmed
      };
      dimmed.into()
    }
  }
}

/// Presses the answer whose number the digit `event` is, from 1 to 9, unless a text field has the focus or a
/// modifier is held.
fn press_digit(presses: &[Option<Handler>], event: &KeyboardEvent) {
  if event.text_input_focused || event.composing || event.ctrl || event.alt || event.meta {
    return;
  }
  let number = match event.key.as_str() {
    "1" => 1,
    "2" => 2,
    "3" => 3,
    "4" => 4,
    "5" => 5,
    "6" => 6,
    "7" => 7,
    "8" => 8,
    "9" => 9,
    _ => return,
  };
  if let Some(Some(press)) = presses.get(number - 1) {
    event.prevent_default();
    press();
  }
}

/// The quiet line under a waiting card: copy, reply, *<agent> asks · <step> · <time>* (`uEaZg`).
fn meta(ctx: &mut Ctx, decision: &Decision) -> Row {
  let id = &decision.id;
  let clipboard = ctx.use_context::<Clipboard>().unwrap_or_default();
  let copied = if decision.words.trim().is_empty() {
    decision.question.to_string()
  } else {
    format!("{}\n\n{}", decision.words, decision.question)
  };
  let copy = pressable(move || {
    if !clipboard.copy(&copied) {
      tracing::warn!("the decision card's text could not be copied to the clipboard");
    }
  })
  .id(format!("{id}-copy"))
  .describe("a11y_name", ctx.t("common.copy"))
  .child(icon(Icon::Copy, IconSize::Base, PaletteColor::TextMuted));
  let reply = decision.reply.as_ref().map(|reply| {
    let reply = reply.0.clone();
    pressable(move || reply())
      .id(format!("{id}-reply"))
      .describe("a11y_name", ctx.t("cards.reply"))
      .child(icon(Icon::MessageSquareReply, IconSize::Base, PaletteColor::TextMuted))
  });
  let asked = &decision.asked;
  let words = ctx.t_args(
    "cards.question.meta",
    [("agent", &*asked.agent), ("step", &*asked.step), ("time", &*asked.time)],
  );
  Row::new()
    .id(format!("{id}-meta"))
    .padding(Padding::horizontal(SpacingSize::Xs))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(copy)
    .with_children(reply)
    .child(Text::new(&words).variant(CardsText::Quiet.role()).nowrap())
}
