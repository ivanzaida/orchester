//! A run's Conversation as Markdown, the whole run with every step: the task's key and title, the date, the task and
//! the run, then under a heading for each step that ran the messages of its agent and the person's, in the order the
//! timeline holds them, each file attached to a message by its name. With tool calls, each command, file edit and MCP
//! call too, with its input and a short result, the problems the transcript recorded, and the step markers: where a
//! return or a restart started an attempt.

use orchester_core::{
  connection::stream::{CommandStatus, FileChangeKind},
  memory::SecretScreen,
  time::Timestamp,
};

use super::{Include, Words, document::Document, stamp};
use crate::{
  messages::Message,
  tasks::{
    conversation::{Author, Entry as MessageEntry},
    timeline::{Item, ItemKind, StepHead, mark::MarkKind},
    transcript::{Entry, EntryKind},
  },
};

/// The task and the run an export is of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunFacts {
  /// The task's key as people read it (`KON-12`) and its title.
  pub key: String,
  pub title: String,
  /// The run's number among the task's runs.
  pub number: u32,
}

impl RunFacts {
  /// The file's name: `kon-12-run-8-conversation.md`.
  pub fn file_name(&self) -> String {
    super::file_name(&format!("{} run {} conversation", self.key, self.number))
  }
}

/// What the dialog says the export covers: the steps that ran and the messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunSpan {
  pub steps: usize,
  pub messages: usize,
}

/// The steps that ran and the messages among `items`.
pub fn span(items: &[Item]) -> RunSpan {
  RunSpan {
    steps: items.iter().filter(|item| ran(item).is_some()).count(),
    messages: items.iter().filter(|item| said(item).is_some()).count(),
  }
}

/// The run of `facts` with its timeline `items` as Markdown with what `include` says, exported at `now`, its words in
/// `words` and the values of the project's secrets masked by `screen`.
pub fn markdown(
  facts: &RunFacts,
  items: &[Item],
  include: Include,
  (words, screen): (Words<'_>, &SecretScreen),
  now: Timestamp,
) -> String {
  let title = format!("{} · {}", facts.key, facts.title);
  let mut file = Document::new(&title, words);
  file.fact("export.markdown.date", &stamp(now));
  file.fact("export.markdown.task", &title);
  let run = file.say(&Message::new("export.markdown.runNumber").with("number", facts.number.to_string()));
  file.fact("export.markdown.run", &run);
  file.rule();
  for item in items {
    if let Some(head) = ran(item) {
      file.end_tools();
      file.step(&format!("{:02} {} · {}", head.number, head.step, head.agent));
    } else if let Some((author, body, names)) = said(item) {
      let author = author_words(&file, &author);
      file.end_tools();
      file.message(&author, item.at, &body, &names);
    } else if include == Include::WithTools {
      tools(&mut file, item);
    }
  }
  screen.mask(&file.finish())
}

/// The step `item` heads, when it ran.
fn ran(item: &Item) -> Option<&StepHead> {
  match &item.kind {
    ItemKind::Step(head) if head.execution.is_some() => Some(head),
    _ => None,
  }
}

/// Who wrote it, as the file names them.
enum Writer<'a> {
  Agent(&'a str),
  Person(&'a Author),
  Orchestrator,
}

/// A message: who wrote it, its words and the names of its files.
type Said<'a> = (Writer<'a>, String, Vec<&'a str>);

/// The message `item` is.
fn said(item: &Item) -> Option<Said<'_>> {
  match &item.kind {
    ItemKind::AgentMessage { agent, text, .. } => Some((Writer::Agent(agent), text.to_string(), Vec::new())),
    ItemKind::Message(message) | ItemKind::Question { entry: message, .. } => Some(message_said(message)),
    ItemKind::Plan(card) => Some((Writer::Agent(&card.agent), card.plan.text.clone(), Vec::new())),
    _ => None,
  }
}

fn message_said(entry: &MessageEntry) -> Said<'_> {
  let names: Vec<&str> = entry
    .attachments
    .iter()
    .map(|attachment| attachment.name().as_str())
    .collect();
  let writer = match (&entry.author, &entry.orchestrator) {
    (Author::Agent(agent), _) => Writer::Agent(agent),
    (_, Some(_)) => Writer::Orchestrator,
    (author, None) => Writer::Person(author),
  };
  // A message of files alone says nothing but their names, which follow it.
  let body = if entry.attachments_only() {
    String::new()
  } else {
    entry.body.to_string()
  };
  (writer, body, names)
}

fn author_words(file: &Document<'_>, writer: &Writer<'_>) -> String {
  match writer {
    Writer::Agent(agent) => (*agent).to_owned(),
    Writer::Orchestrator => file.say(&Message::new("export.markdown.orchestrator")),
    Writer::Person(Author::YouTo(agent)) => {
      file.say(&Message::new("export.markdown.youTo").with("agent", agent.to_string()))
    }
    Writer::Person(_) => file.say(&Message::new("export.markdown.you")),
  }
}

/// The tool calls of a tool group, and a problem the transcript recorded, each with its input and a short result; and
/// the marks of the attempts a return or a restart started.
fn tools(file: &mut Document<'_>, item: &Item) {
  match &item.kind {
    ItemKind::Tools(group) => {
      for entry in &group.entries {
        tool(file, entry);
      }
    }
    ItemKind::Problem(entry) => tool(file, entry),
    ItemKind::Mark(mark) => {
      let (what, note) = match &mark.kind {
        MarkKind::Returned { to, failed, .. } => (
          Message::new("export.markdown.mark.returned")
            .with("to", to.to_string())
            .with("failed", failed.to_string()),
          "",
        ),
        MarkKind::Restarted { step } => (
          Message::new("export.markdown.mark.restarted").with("step", step.to_string()),
          "",
        ),
        MarkKind::ReturnedWithNote { to, note, .. } => (
          Message::new("export.markdown.mark.returnedWithNote").with("to", to.to_string()),
          &**note,
        ),
      };
      let what = file.say(&what);
      file.end_tools();
      file.tool(&what, "", note);
      file.end_tools();
    }
    _ => {}
  }
}

fn tool(file: &mut Document<'_>, entry: &Entry) {
  let say = |file: &Document<'_>, key: &'static str| file.say(&Message::new(key));
  match &entry.kind {
    EntryKind::Command(command) => {
      let what = say(file, command_key(command.status, command.exit_code));
      let what = with_exit(file, what, command.exit_code);
      file.tool(&what, &command.command, &command.output);
    }
    EntryKind::ToolCommand(command) => {
      let what = with_exit(file, say(file, "export.markdown.tool.command"), command.exit_code);
      file.tool(&what, &command.command, &command.output);
    }
    EntryKind::FileChange(change) => {
      let key = match change.kind {
        FileChangeKind::Added => "export.markdown.tool.added",
        FileChangeKind::Modified => "export.markdown.tool.edited",
        FileChangeKind::Deleted => "export.markdown.tool.deleted",
      };
      file.tool(&say(file, key), &change.path, "");
    }
    EntryKind::McpTool(call) => {
      let what = file.say(&Message::new("export.markdown.tool.mcp").with("server", call.server.to_string()));
      file.tool(&what, &call.tool, "");
    }
    EntryKind::Error(text) | EntryKind::NotSignedIn(text) => {
      file.tool(&say(file, "export.markdown.tool.error"), "", text);
    }
    _ => {}
  }
}

/// A command as it went: declined, failed, or run.
const fn command_key(status: Option<CommandStatus>, exit: Option<i64>) -> &'static str {
  match (status, exit) {
    (Some(CommandStatus::Declined), _) => "export.markdown.tool.declined",
    (Some(CommandStatus::Failed), None) => "export.markdown.tool.failed",
    _ => "export.markdown.tool.command",
  }
}

/// `what` with the exit code the command ended with, when it reported one.
fn with_exit(file: &Document<'_>, what: String, exit: Option<i64>) -> String {
  match exit {
    Some(code) => file.say(
      &Message::new("export.markdown.tool.exited")
        .with("what", what)
        .with("code", code.to_string()),
    ),
    None => what,
  }
}
