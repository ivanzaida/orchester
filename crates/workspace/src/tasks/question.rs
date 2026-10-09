//! Answering an agent's question with suggested answers (`docs/design/orchester-workspace.md`, "Questions
//! with options"), as the card in the task conversation and Orchester's MCP server both do: the answers go
//! to the runner ([`command`]), which checks them against the questions and gives them to the asking
//! agent. [`Draft`] is what the person picked and typed on a card before *Answer*.

use orchester_core::{
  id::RunId,
  question::{AnswerError, GivenAnswer, Question},
  run::Answerer,
};
use orchester_ipc::Command;

use crate::messages::Message;

/// Answers to one request: its run, its id and one answer per question.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answering {
  pub run: RunId,
  pub request: String,
  pub answers: Vec<GivenAnswer>,
}

impl Answering {
  /// The runner command that gives these answers, by `by`.
  pub fn command(self, by: Answerer) -> Command {
    Command::AnswerQuestion {
      run: self.run,
      request: self.request,
      answers: self.answers,
      by,
    }
  }
}

impl lurq::app::component::DevtoolsInspectable for Answering {}

/// Why the runner refused answers, in the words the app and the MCP server use.
pub fn answer_error(error: &AnswerError) -> Message {
  match error {
    AnswerError::Empty => Message::new("runs.conversation.question.errors.empty"),
    AnswerError::UnknownChoice { label } => {
      Message::new("runs.conversation.question.errors.unknownChoice").with("label", label.clone())
    }
    AnswerError::RepeatedChoice { label } => {
      Message::new("runs.conversation.question.errors.repeatedChoice").with("label", label.clone())
    }
    AnswerError::SeveralForSingle => Message::new("runs.conversation.question.errors.severalForSingle"),
    AnswerError::AnswerCount { expected, .. } => {
      Message::new("runs.conversation.question.errors.answerCount").with("count", expected.to_string())
    }
  }
}

/// One question's picks on a card: the options picked, and the person's own answer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Picks {
  /// Indexes of the options picked, in the options' order.
  options: Vec<usize>,
  /// The own answer's row is picked: typing in its field picks it.
  own: bool,
  text: String,
}

impl Picks {
  pub fn picked(&self, option: usize) -> bool {
    self.options.contains(&option)
  }

  pub const fn own_picked(&self) -> bool {
    self.own
  }

  pub fn text(&self) -> &str {
    &self.text
  }

  /// Whether this question has an answer: an option picked, or an own answer typed.
  pub fn answered(&self) -> bool {
    !self.options.is_empty() || (self.own && !self.text.trim().is_empty())
  }
}

/// What the person picked and typed on a card, one [`Picks`] per question of the request.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Draft {
  picks: Vec<Picks>,
}

impl lurq::app::component::DevtoolsInspectable for Draft {}

impl Draft {
  /// Nothing picked yet, for `questions`.
  pub fn new(questions: &[Question]) -> Self {
    Self {
      picks: vec![Picks::default(); questions.len()],
    }
  }

  pub fn of(&self, question: usize) -> Option<&Picks> {
    self.picks.get(question)
  }

  /// Picks option `option` of question `index`: with several allowed it checks or unchecks it, otherwise it
  /// replaces the pick (the own answer's too).
  pub fn pick(&mut self, questions: &[Question], (index, option): (usize, usize)) {
    let (Some(question), Some(picks)) = (questions.get(index), self.picks.get_mut(index)) else {
      return;
    };
    if option >= question.options().len() {
      return;
    }
    if question.multi_select() {
      match picks.options.binary_search(&option) {
        Ok(at) => {
          picks.options.remove(at);
        }
        Err(at) => picks.options.insert(at, option),
      }
    } else {
      picks.options = vec![option];
      picks.own = false;
    }
  }

  /// The own answer of question `index` reads `text`: typing picks its row (replacing the other pick of a
  /// single-choice question); emptying it unpicks the row.
  pub fn type_own(&mut self, questions: &[Question], index: usize, text: &str) {
    let (Some(question), Some(picks)) = (questions.get(index), self.picks.get_mut(index)) else {
      return;
    };
    picks.text = text.to_owned();
    picks.own = !text.trim().is_empty();
    if picks.own && !question.multi_select() {
      picks.options.clear();
    }
  }

  /// The first question without an answer, by its index; `None` once every question has one.
  pub fn missing(&self) -> Option<usize> {
    self.picks.iter().position(|picks| !picks.answered())
  }

  /// The answers as the runner takes them: the labels picked in order, and the own text while its row is
  /// picked.
  pub fn given(&self, questions: &[Question]) -> Vec<GivenAnswer> {
    questions
      .iter()
      .zip(&self.picks)
      .map(|(question, picks)| GivenAnswer {
        choices: picks
          .options
          .iter()
          .filter_map(|option| question.options().get(*option))
          .map(|option| option.label().to_owned())
          .collect(),
        text: picks
          .own
          .then(|| picks.text.trim().to_owned())
          .filter(|text| !text.is_empty()),
      })
      .collect()
  }
}

/// The letter of option `index`, as the card shows it: A, B, C…; past Z, none.
pub fn letter(index: usize) -> Option<char> {
  u8::try_from(index)
    .ok()
    .filter(|index| *index < 26)
    .map(|index| char::from(b'A' + index))
}

/// The option a letter key picks, `key` read case-insensitively; `options` is how many the question has, and
/// the letter after the last is its own answer (`Some(options)`).
pub fn option_of_key(key: &str, options: usize) -> Option<usize> {
  let mut chars = key.chars();
  let (Some(first), None) = (chars.next(), chars.next()) else {
    return None;
  };
  let first = first.to_ascii_uppercase();
  if !first.is_ascii_uppercase() {
    return None;
  }
  let index = usize::from(u8::try_from(first).ok()? - b'A');
  (index <= options).then_some(index)
}
