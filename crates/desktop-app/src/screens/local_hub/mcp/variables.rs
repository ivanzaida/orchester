//! *Variables* as the panel shows them (L2 `IpzZ7`, L12): a variable that takes a secret reads `FIGMA_API_KEY ← secret
//! FIGMA_TOKEN`, the others `NAME=value`, while the field the server is saved from keeps what is stored,
//! `FIGMA_API_KEY=${secret:FIGMA_TOKEN@development}`. The two are kept in step both ways: what is typed in the shown
//! form is written back with the reference the variable held (or another variable's reference to the same secret), and
//! a reference written into the field (*Add a variable from a secret…*) shows in the shown form.

use lurq::{app::ctx::Ctx, core::Signal};

/// How a secret's reference opens and closes in a stored line: `${secret:NAME@environment}`.
const OPEN: &str = "${secret:";
const CLOSE: char = '}';

/// The words of `NAME ← secret SECRET` around the variable's and the secret's names, from the locale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens::local_hub) struct Notation {
  head: String,
  between: String,
  tail: String,
}

impl Notation {
  pub(in crate::screens::local_hub) fn new(ctx: &Ctx) -> Self {
    // Markers no name holds stand for the names, so the words around them can be cut out.
    let (name, secret) = ("\u{1}", "\u{2}");
    let words = ctx.t_args("local_hub.mcp.takesSecret", [("name", name), ("secret", secret)]);
    let (head, rest) = words.split_once(name).unwrap_or(("", &words));
    let (between, tail) = rest.split_once(secret).unwrap_or((rest, ""));
    Self {
      head: head.to_owned(),
      between: between.to_owned(),
      tail: tail.to_owned(),
    }
  }

  /// What the field shows of the stored `text`.
  pub(in crate::screens::local_hub) fn shown(&self, text: &str) -> String {
    text
      .lines()
      .map(|line| match reference(line) {
        Some((name, secret, _)) => format!("{}{name}{}{secret}{}", self.head, self.between, self.tail),
        None => line.to_owned(),
      })
      .collect::<Vec<_>>()
      .join("\n")
  }

  /// What is stored of the `shown` lines, `before` the stored text they were shown from: a variable in the shown form
  /// takes the reference `before` held for its secret; one whose secret `before` does not name stays as typed, which
  /// the field's check refuses.
  pub(super) fn stored(&self, shown: &str, before: &str) -> String {
    shown
      .lines()
      .map(|line| match self.parse(line) {
        Some((name, secret)) => before
          .lines()
          .filter_map(reference)
          .find(|(_, held, _)| *held == secret)
          .map_or_else(|| line.to_owned(), |(_, _, value)| format!("{name}={value}")),
        None => line.to_owned(),
      })
      .collect::<Vec<_>>()
      .join("\n")
  }

  /// The variable's and the secret's names of a line in the shown form.
  fn parse<'a>(&self, line: &'a str) -> Option<(&'a str, &'a str)> {
    let inner = line
      .strip_prefix(self.head.as_str())?
      .strip_suffix(self.tail.as_str())?;
    let (name, secret) = inner.split_once(self.between.as_str())?;
    (!name.is_empty() && !secret.is_empty()).then_some((name, secret))
  }
}

/// The variable's name, the secret's name and the whole reference of a stored line that takes a secret.
fn reference(line: &str) -> Option<(&str, &str, &str)> {
  let (name, value) = line.split_once('=')?;
  let held = value.strip_prefix(OPEN)?.strip_suffix(CLOSE)?;
  let secret = held.split_once('@').map_or(held, |(secret, _)| secret);
  Some((name, secret, value))
}

/// The shown form of `variables`, kept in step with it both ways.
pub(super) fn line(ctx: &mut Ctx, variables: &Signal<String>) -> Signal<String> {
  let notation = Notation::new(ctx);
  let line = ctx.signal(notation.shown(&variables.get()));
  let (field, words) = (variables.clone(), notation.clone());
  ctx.watch(&line, move |typed| {
    let stored = words.stored(typed, &field.get());
    if field.get() != stored {
      field.set(stored);
    }
  });
  let shown = line.clone();
  ctx.watch(variables, move |text| {
    let now = notation.shown(text);
    if shown.get() != now {
      shown.set(now);
    }
  });
  line
}
