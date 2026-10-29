//! The inline decision card every decision of a run's conversation shares — a question, a plan, a command approval
//! (`Conversation cards · redesign` `p2o1w`, `docs/design/conversation-cards.md`) — and its answered line ([`line`]).
//!
//! The card has no fill, a `border` stroke and radius 10 (`yYTFc` in 05 `yrI3M`): the agent's own words in muted
//! 13 · 1.55, folded at three lines behind *Show the whole question · N lines* (05b `L7AgD` e); the decision in one
//! 15/600 line; then the answers, each a primary (the first) or secondary button with its key: those with a
//! consequence stack at full width, the others sit in one row (05b a). One click answers; the digits 1–9 press the
//! matching answer while no text field has the focus. ✕ in the corner folds the card to *Waits for your answer*
//! (05c `hcN1X` i). Under the card is one quiet meta line: copy, reply, *<agent> asks · <step> · <time>*.
//!
//! Answered, the card is its [`AnsweredLine`]; a press on it opens the card as it was, the chosen answer checked and
//! the others dimmed, with a footer under them (05c a–b).

mod card;
mod line;

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};

use self::card::DecisionView;
pub use self::line::{AnsweredLine, Mark};

/// What a press on an answer does.
pub type Handler = Arc<dyn Fn() + Send + Sync>;

/// One answer of a card.
#[derive(Clone)]
pub struct Answer {
  label: Arc<str>,
  consequence: Option<Arc<str>>,
  /// The person answers in their own words: a pencil before the label, and left out once the card is answered.
  own: bool,
  press: Option<Handler>,
}

impl Answer {
  /// An answer worded `label`; without [`Self::on_press`] it cannot be pressed.
  pub fn new(label: impl Into<Arc<str>>) -> Self {
    Self {
      label: label.into(),
      consequence: None,
      own: false,
      press: None,
    }
  }

  /// *Answer in my own words* and its kin, worded `label`: a pencil before it (`XCtz2`); it shows only while the
  /// card waits.
  pub fn own(label: impl Into<Arc<str>>) -> Self {
    Self {
      own: true,
      ..Self::new(label)
    }
  }

  /// What choosing it leads to, under its label: the answer is then as wide as the card (`wWuSM`).
  #[must_use]
  pub fn consequence(mut self, words: impl Into<Arc<str>>) -> Self {
    self.consequence = Some(words.into());
    self
  }

  #[must_use]
  pub fn on_press(mut self, press: impl Fn() + Send + Sync + 'static) -> Self {
    self.press = Some(Arc::new(press));
    self
  }
}

impl PartialEq for Answer {
  /// What a press does is the caller's; only whether it can be pressed counts.
  fn eq(&self, other: &Self) -> bool {
    (&self.label, &self.consequence, self.own, self.press.is_some())
      == (&other.label, &other.consequence, other.own, other.press.is_some())
  }
}

/// Who asked, in which step and when: the meta line under a waiting card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asked {
  pub agent: Arc<str>,
  /// "03 Implement".
  pub step: Arc<str>,
  /// The clock time it was asked, "15:21".
  pub time: Arc<str>,
}

/// Where the decision stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stage {
  /// It waits for an answer.
  Waiting,
  /// It was decided, or ended without a decision: the card is `line`, which opens it with the `chosen` answer
  /// checked (none, when no answer of the card was chosen) and `footer` under the answers.
  Answered {
    line: AnsweredLine,
    chosen: Option<usize>,
    footer: Option<Arc<str>>,
  },
}

/// An inline decision card, built and then shown with [`Self::render`].
#[derive(Clone, PartialEq)]
pub struct Decision {
  id: Arc<str>,
  words: Arc<str>,
  question: Arc<str>,
  answers: Vec<Answer>,
  asked: Asked,
  stage: Stage,
  reply: Option<ReplyHandler>,
}

/// What the meta line's reply does.
#[derive(Clone)]
struct ReplyHandler(Handler);

impl PartialEq for ReplyHandler {
  fn eq(&self, _: &Self) -> bool {
    true
  }
}

impl Decision {
  /// The card `id` asking `question`, after the agent's own `words` (none when empty), `asked` as it says.
  /// Its parts are `<id>-card`, `<id>-words`, `<id>-whole`, `<id>-question`, `<id>-fold`, `<id>-answer-<n>` (from
  /// 1), `<id>-meta`, `<id>-copy`, `<id>-reply`, `<id>-line` and `<id>-footer`.
  pub fn new(id: impl Into<Arc<str>>, words: impl Into<Arc<str>>, question: impl Into<Arc<str>>, asked: Asked) -> Self {
    Self {
      id: id.into(),
      words: words.into(),
      question: question.into(),
      answers: Vec::new(),
      asked,
      stage: Stage::Waiting,
      reply: None,
    }
  }

  /// The answers in the agent's order: the first is the primary one.
  #[must_use]
  pub fn answers(mut self, answers: impl IntoIterator<Item = Answer>) -> Self {
    self.answers = answers.into_iter().collect();
    self
  }

  #[must_use]
  pub fn stage(mut self, stage: Stage) -> Self {
    self.stage = stage;
    self
  }

  /// The meta line's reply runs `reply`, e.g. taking the person to the composer; without it there is no reply.
  #[must_use]
  pub fn on_reply(mut self, reply: impl Fn() + Send + Sync + 'static) -> Self {
    self.reply = Some(ReplyHandler(Arc::new(reply)));
    self
  }

  /// The card, keeping whether it is folded and its words shown whole while it stays mounted.
  pub fn render(self, ctx: &mut Ctx) -> Element {
    let key = format!("decision-{}", self.id);
    ctx.mount_keyed::<DecisionView>(&key, self)
  }
}

impl lurq::app::component::DevtoolsInspectable for Decision {}
