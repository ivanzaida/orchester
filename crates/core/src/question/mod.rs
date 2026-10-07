//! A question an agent asks the person with suggested answers (`docs/design/orchester-workspace.md`,
//! "Questions with options"): its text, an optional short header, at least two options (a label and an
//! optional description each) and whether several may be picked. The person may always type their own
//! answer instead of, or besides, the options. An answer is the labels picked plus that text.
//!
//! Claude Code asks through Orchester's `ask_question` tool, Codex with `item/tool/requestUserInput` (one
//! request may hold several questions), a native agent with its own `ask_question`. A plain question
//! without options stays the step result that ends the turn (`crate::run::StepOutcome::Question`).

mod request;
pub mod tool;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use self::request::{AskedQuestions, QuestionRequest, QuestionState, QuestionTurn};

/// The most options one question may suggest.
pub const MAX_OPTIONS: usize = 26;

/// One suggested answer: a label the agent gets back when it is picked, and what it means.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionOption {
  label: String,
  description: Option<String>,
}

impl QuestionOption {
  /// An option with the whitespace around its label and description removed; a blank description is none.
  pub fn new(label: &str, description: Option<&str>) -> Self {
    Self {
      label: label.trim().to_owned(),
      description: description
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned),
    }
  }

  pub fn label(&self) -> &str {
    &self.label
  }

  pub fn description(&self) -> Option<&str> {
    self.description.as_deref()
  }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum QuestionError {
  #[error("the question must not be blank")]
  BlankText,
  #[error("a question with options suggests at least two")]
  TooFewOptions,
  #[error("a question suggests at most {MAX_OPTIONS} options")]
  TooManyOptions,
  #[error("an option's label must not be blank")]
  BlankLabel,
  #[error("two options are labelled `{0}`; each label must be different")]
  RepeatedLabel(String),
}

/// A question with suggested answers, valid by construction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "QuestionParts", into = "QuestionParts")]
pub struct Question {
  text: String,
  header: Option<String>,
  options: Vec<QuestionOption>,
  multi_select: bool,
}

impl Question {
  /// A question `text`, under an optional short `header`, suggesting `options` (at least two, each label
  /// different, compared without regard to case); with `multi_select` several may be picked.
  pub fn new(
    text: &str,
    header: Option<&str>,
    options: Vec<QuestionOption>,
    multi_select: bool,
  ) -> Result<Self, QuestionError> {
    let text = text.trim();
    if text.is_empty() {
      return Err(QuestionError::BlankText);
    }
    if options.len() < 2 {
      return Err(QuestionError::TooFewOptions);
    }
    if options.len() > MAX_OPTIONS {
      return Err(QuestionError::TooManyOptions);
    }
    for (index, option) in options.iter().enumerate() {
      if option.label.is_empty() {
        return Err(QuestionError::BlankLabel);
      }
      if options[..index]
        .iter()
        .any(|earlier| earlier.label.eq_ignore_ascii_case(&option.label))
      {
        return Err(QuestionError::RepeatedLabel(option.label.clone()));
      }
    }
    Ok(Self {
      text: text.to_owned(),
      header: header
        .map(str::trim)
        .filter(|header| !header.is_empty())
        .map(str::to_owned),
      options,
      multi_select,
    })
  }

  pub fn text(&self) -> &str {
    &self.text
  }

  pub fn header(&self) -> Option<&str> {
    self.header.as_deref()
  }

  pub fn options(&self) -> &[QuestionOption] {
    &self.options
  }

  /// Several options may be picked.
  pub const fn multi_select(&self) -> bool {
    self.multi_select
  }

  /// The answer that picks `choices` (option labels, each once) and says `text` in the person's own
  /// words. Refused: no choice and no text, a label the question does not suggest, a label given twice,
  /// and more than one answer to a single-choice question, where an own text replaces a pick.
  pub fn answer(&self, choices: &[String], text: Option<&str>) -> Result<Answer, AnswerError> {
    let text = text.map(str::trim).filter(|text| !text.is_empty());
    if choices.is_empty() && text.is_none() {
      return Err(AnswerError::Empty);
    }
    if !self.multi_select && choices.len() + usize::from(text.is_some()) > 1 {
      return Err(AnswerError::SeveralForSingle);
    }
    let mut picked: Vec<String> = Vec::with_capacity(choices.len());
    for choice in choices {
      let option = self
        .options
        .iter()
        .find(|option| option.label == choice.trim())
        .ok_or_else(|| AnswerError::UnknownChoice { label: choice.clone() })?;
      if picked.contains(&option.label) {
        return Err(AnswerError::RepeatedChoice {
          label: option.label.clone(),
        });
      }
      picked.push(option.label.clone());
    }
    Ok(Answer {
      choices: picked,
      text: text.map(str::to_owned),
    })
  }
}

/// How a [`Question`] is stored; read back through [`Question::new`].
#[derive(Serialize, Deserialize)]
struct QuestionParts {
  text: String,
  header: Option<String>,
  options: Vec<QuestionOption>,
  multi_select: bool,
}

impl TryFrom<QuestionParts> for Question {
  type Error = QuestionError;

  fn try_from(parts: QuestionParts) -> Result<Self, QuestionError> {
    Self::new(&parts.text, parts.header.as_deref(), parts.options, parts.multi_select)
  }
}

impl From<Question> for QuestionParts {
  fn from(question: Question) -> Self {
    Self {
      text: question.text,
      header: question.header,
      options: question.options,
      multi_select: question.multi_select,
    }
  }
}

/// Why an answer is refused; what the app and Orchester's MCP server say.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnswerError {
  #[error("an answer picks an option or says something")]
  Empty,
  #[error("the question suggests no option labelled `{label}`")]
  UnknownChoice { label: String },
  #[error("`{label}` is picked twice")]
  RepeatedChoice { label: String },
  #[error("the question takes one answer: one option, or an own answer")]
  SeveralForSingle,
  #[error("the request asks {expected} questions; {given} answers were given")]
  AnswerCount { expected: usize, given: usize },
}

/// An answer as given, before it is checked against its question ([`Question::answer`]): the labels picked
/// and the own text.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GivenAnswer {
  pub choices: Vec<String>,
  pub text: Option<String>,
}

impl From<&Answer> for GivenAnswer {
  fn from(answer: &Answer) -> Self {
    Self {
      choices: answer.choices.clone(),
      text: answer.text.clone(),
    }
  }
}

/// The person's answer to one question: the labels picked, in the order picked, and their own text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer {
  choices: Vec<String>,
  text: Option<String>,
}

impl Answer {
  pub fn choices(&self) -> &[String] {
    &self.choices
  }

  /// What the person typed as their own answer.
  pub fn text(&self) -> Option<&str> {
    self.text.as_deref()
  }

  /// The answer as one list of strings: the labels picked, then the own text.
  pub fn values(&self) -> Vec<String> {
    let mut values = self.choices.clone();
    values.extend(self.text.clone());
    values
  }

  /// What an agent reads of it: each pick on a line, then the own answer.
  pub fn for_agent(&self) -> String {
    let mut lines: Vec<String> = self.choices.iter().map(|choice| format!("- {choice}")).collect();
    if let Some(text) = &self.text {
      lines.push(format!("- In their own words: {text}"));
    }
    lines.join("\n")
  }
}

/// What an agent reads of the answers to `questions`: one question's answer alone, several each under
/// its question.
pub fn answers_for_agent(questions: &[Question], answers: &[Answer]) -> String {
  if let ([_], [answer]) = (questions, answers) {
    return format!("The person answered:\n{}", answer.for_agent());
  }
  let parts: Vec<String> = questions
    .iter()
    .zip(answers)
    .enumerate()
    .map(|(index, (question, answer))| format!("{}. {}\n{}", index + 1, question.text(), answer.for_agent()))
    .collect();
  format!("The person answered:\n\n{}", parts.join("\n\n"))
}

/// What a person reads of `question` where no card shows it: the question, then its options.
pub fn question_as_text(question: &Question) -> String {
  let options: Vec<String> = question
    .options()
    .iter()
    .map(|option| match option.description() {
      Some(description) => format!("- {}: {description}", option.label()),
      None => format!("- {}", option.label()),
    })
    .collect();
  format!("{}\n\n{}", question.text(), options.join("\n"))
}
