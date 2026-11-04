//! Finding a word in a run's timeline (ORC-306, `Task page · redesign` `C3wC8` 13–13f): every place a plain text is
//! written in the run, case and accents ignored, in the order the chat shows them. It searches what the person and the
//! agents wrote (as the chat shows their Markdown), the cards, problems, notes and notices, and in tool groups each
//! call's command, path or tool, what it said and every line of its output; usage reports say nothing to find. Each
//! place found ([`Hit`]) names the item it is in, the step its fold folds it into, and for a call the call and the
//! line, so the chat can open what hides it. The words Orchester puts around them (a step's divider, a group's summary)
//! are not searched: they are no text of the run.

use std::{borrow::Cow, ops::Range, sync::Arc};

use super::{Item, ItemKind, mark::MarkKind, view::Rendered};
use crate::{
  mentions::facts,
  tasks::transcript::{Entry, EntryKind, command_preview, in_worktree},
};

/// At most this many places are found in one run; a search that finds more says so.
pub const MAX_HITS: usize = 10_000;
/// How many items are searched between asking whether the search is still wanted.
const WANTED_EVERY: usize = 64;

/// What to look for: the query's characters, folded ([`fold`]), without the spaces around it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Needle(Vec<char>);

impl Needle {
  /// The needle of `query`; `None` when it is blank.
  pub fn new(query: &str) -> Option<Self> {
    let query = query.trim();
    (!query.is_empty()).then(|| Self(query.chars().map(fold).collect()))
  }

  /// The byte ranges of `text` where the needle is, from its start, none overlapping.
  pub fn places(&self, text: &str) -> Vec<Range<usize>> {
    let chars: Vec<(usize, char)> = text.char_indices().map(|(at, c)| (at, fold(c))).collect();
    let length = self.0.len();
    let mut found = Vec::new();
    let mut start = 0;
    while start + length <= chars.len() {
      if chars[start..start + length]
        .iter()
        .map(|(_, c)| *c)
        .eq(self.0.iter().copied())
      {
        let end = chars.get(start + length).map_or(text.len(), |(at, _)| *at);
        found.push(chars[start].0..end);
        start += length;
      } else {
        start += 1;
      }
    }
    found
  }
}

/// `character` as a search compares it: in lower case, a Latin letter without its accent (`É` is `e`, `ł` is `l`),
/// `ё` as `е`. Each character stays one, so a place found is a place in the text as written.
pub fn fold(character: char) -> char {
  let lower = character.to_lowercase().next().unwrap_or(character);
  match lower {
    'à'..='å' | 'ā' | 'ă' | 'ą' => 'a',
    'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
    'ď' | 'đ' => 'd',
    'è'..='ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
    'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
    'ĥ' | 'ħ' => 'h',
    'ì'..='ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => 'i',
    'ĵ' => 'j',
    'ķ' => 'k',
    'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => 'l',
    'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
    'ò'..='ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => 'o',
    'ŕ' | 'ŗ' | 'ř' => 'r',
    'ś' | 'ŝ' | 'ş' | 'š' => 's',
    'ţ' | 'ť' | 'ŧ' => 't',
    'ù'..='ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
    'ŵ' => 'w',
    'ý' | 'ÿ' | 'ŷ' => 'y',
    'ź' | 'ż' | 'ž' => 'z',
    'ё' => 'е',
    other => other,
  }
}

/// Where in an item or a call a place is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Spot {
  /// What an item says: a message, a card, a problem, a note, a notice.
  Said,
  /// A call's line: its command on one line, its path, as its row shows it.
  Title,
  /// What a call says under its line when it is open: what the agent reasoned or wrote, an MCP call's server and tool.
  Detail,
  /// Line `n` of a call's output, from 0.
  Output(usize),
}

/// The call of a tool group a place is in.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InCall {
  /// The call's row key, which keeps whether it is open (`entry-<sequence>-<part>`).
  pub key: Arc<str>,
  /// Its place among the calls its group lists (usage reports left out), from 0.
  pub rank: usize,
}

/// A place the needle was found.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Hit {
  /// The key of the item it is in: for a call, its tool group's.
  pub item: Arc<str>,
  /// The key of the step whose fold hides the item, while it is in a step.
  pub step: Option<Arc<str>>,
  pub call: Option<InCall>,
  pub spot: Spot,
  /// Its bytes in the text of its spot.
  pub range: Range<usize>,
}

impl Hit {
  /// Whether `other` is this place in a timeline read again: the same text, at the same bytes.
  pub fn same_place(&self, other: &Self) -> bool {
    self.item == other.item
      && self.call.as_ref().map(|call| &call.key) == other.call.as_ref().map(|call| &call.key)
      && self.spot == other.spot
      && self.range.start == other.range.start
  }
}

/// The row key of a call in a tool group: whether a person opened it is kept by it.
pub fn call_key(entry: &Entry) -> Arc<str> {
  format!("entry-{}-{}", entry.sequence, entry.part).into()
}

/// Every place `needle` is in `items`, in the order the chat shows them, at most [`MAX_HITS`]; what agents and the
/// person wrote is also found as `rendered` shows it. `wanted` is asked as it goes: once it says no (a newer search
/// superseded this one), the search stops and finds nothing.
pub fn find(items: &[Item], needle: &Needle, rendered: Rendered<'_>, wanted: &dyn Fn() -> bool) -> Option<Vec<Hit>> {
  let mut search = Search {
    needle,
    rendered,
    hits: Vec::new(),
  };
  let mut step: Option<Arc<str>> = None;
  for (index, item) in items.iter().enumerate() {
    if index % WANTED_EVERY == 0 && !wanted() {
      return None;
    }
    match &item.kind {
      ItemKind::Step(head) => step = head.execution.is_some().then(|| item.key.clone()),
      ItemKind::Mark(_) => step = None,
      _ if item.execution.is_none() => step = None,
      _ => {}
    }
    search.item(item, step.as_ref());
    if search.hits.len() >= MAX_HITS {
      search.hits.truncate(MAX_HITS);
      break;
    }
  }
  Some(search.hits)
}

/// One search over the items.
struct Search<'a> {
  needle: &'a Needle,
  rendered: Rendered<'a>,
  hits: Vec<Hit>,
}

impl Search<'_> {
  fn item(&mut self, item: &Item, step: Option<&Arc<str>>) {
    if let ItemKind::Tools(group) = &item.kind {
      let calls = group
        .entries
        .iter()
        .filter(|entry| !matches!(entry.kind, EntryKind::Usage(_)));
      for (rank, entry) in calls.enumerate() {
        let call = InCall {
          key: call_key(entry),
          rank,
        };
        for (spot, text) in call_texts(entry) {
          self.add((item, step), (Some(&call), spot), &text);
        }
      }
      return;
    }
    for text in said(item, self.rendered) {
      self.add((item, step), (None, Spot::Said), &text);
    }
  }

  fn add(&mut self, (item, step): (&Item, Option<&Arc<str>>), (call, spot): (Option<&InCall>, Spot), text: &str) {
    for range in self.needle.places(text) {
      self.hits.push(Hit {
        item: item.key.clone(),
        step: step.cloned(),
        call: call.cloned(),
        spot,
        range,
      });
    }
  }
}

/// What `item` says, as the chat shows it: Markdown as `rendered` shows it, a person's message without the block of
/// facts Orchester added for the agent about what it mentions (`mentions::facts`), which the chat does not show.
fn said<'a>(item: &'a Item, rendered: Rendered<'_>) -> Vec<Cow<'a, str>> {
  let shown = |text: &'a str| -> Cow<'a, str> { rendered(text).map_or(Cow::Borrowed(text), Cow::Owned) };
  let plain = |text: &'a str| Cow::Borrowed(text);
  match &item.kind {
    ItemKind::AgentMessage { text, .. } => vec![shown(text)],
    ItemKind::Message(entry) if entry.orchestrator.is_none() => vec![shown(facts::said(&entry.body))],
    ItemKind::Message(entry) | ItemKind::Question { entry, .. } => vec![shown(&entry.body)],
    ItemKind::Plan(card) => vec![shown(&card.plan.text)],
    ItemKind::Approval(card) => [
      Some(card.request.action.as_str()),
      Some(card.request.server.as_str()),
      card.request.target.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(plain)
    .collect(),
    ItemKind::Choice(card) => card
      .asked
      .request
      .questions()
      .iter()
      .flat_map(|question| {
        std::iter::once(question.text()).chain(
          question
            .options()
            .iter()
            .flat_map(|option| std::iter::once(option.label()).chain(option.description())),
        )
      })
      .map(plain)
      .collect(),
    ItemKind::Problem(entry) => call_texts(entry).into_iter().map(|(_, text)| text).collect(),
    ItemKind::Result(card) => card.reported.as_deref().map(shown).into_iter().collect(),
    ItemKind::Mark(mark) => match &mark.kind {
      MarkKind::ReturnedWithNote { note, .. } => vec![shown(note)],
      MarkKind::Returned { .. } | MarkKind::Restarted { .. } => Vec::new(),
    },
    ItemKind::BaseMoved(mark) => {
      let notice = &mark.notice;
      std::iter::once(plain(notice.branch.as_str()))
        .chain(
          notice
            .tasks
            .iter()
            .flat_map(|task| [plain(&task.key), plain(&task.title)]),
        )
        .chain(notice.commits.iter().map(|commit| plain(&commit.subject)))
        .chain(notice.overlapping.iter().map(|path| plain(path)))
        .collect()
    }
    ItemKind::Tools(_)
    | ItemKind::Step(_)
    | ItemKind::SwitchedToManual(_)
    | ItemKind::Permission(_)
    | ItemKind::Model(_)
    | ItemKind::BackToProfile(_)
    | ItemKind::End(_) => Vec::new(),
  }
}

/// What a call says, spot by spot: its line, what it says when open, its output line by line.
fn call_texts(entry: &Entry) -> Vec<(Spot, Cow<'_, str>)> {
  let output = |output: &str| -> Vec<(Spot, Cow<'_, str>)> {
    output
      .lines()
      .enumerate()
      .map(|(line, text)| (Spot::Output(line), Cow::Owned(text.to_owned())))
      .collect()
  };
  let detail = |text: &str| (Spot::Detail, Cow::Owned(text.to_owned()));
  match &entry.kind {
    EntryKind::Command(command) => {
      let mut texts = vec![(Spot::Title, command_preview(&command.command))];
      texts.extend(output(&command.output));
      texts
    }
    EntryKind::ToolCommand(command) => {
      let mut texts = vec![(Spot::Title, command_preview(&command.command))];
      texts.extend(output(&command.output));
      texts
    }
    EntryKind::FileChange(change) => {
      let path = in_worktree(&change.path).unwrap_or(&change.path).replace('\\', "/");
      vec![(Spot::Title, Cow::Owned(path))]
    }
    EntryKind::Message(text) | EntryKind::Reasoning(text) | EntryKind::Error(text) => vec![detail(text)],
    EntryKind::NotSignedIn(text) => vec![(Spot::Output(0), Cow::Owned(text.to_string()))],
    EntryKind::Raw(lines) => lines
      .iter()
      .enumerate()
      .map(|(line, (_, text))| (Spot::Output(line), Cow::Owned(text.to_string())))
      .collect(),
    EntryKind::McpTool(call) => vec![detail(&call.server), detail(&call.tool)],
    EntryKind::McpServer(server, _) => server.as_deref().map(detail).into_iter().collect(),
    EntryKind::ExemptionNotApplied { server, tool, .. } => vec![detail(server), detail(tool)],
    EntryKind::Approval(approval) => vec![detail(&approval.command)],
    EntryKind::Usage(_) | EntryKind::Problem(_) | EntryKind::TurnStopped => Vec::new(),
  }
}
