//! The form of a new MCP server (`Agents · New MCP server` A8 `vPoTQ`, A9 `ajhYK` for a name the team has): what is the
//! same on every computer, which is how it runs (a program's name, never its path, with its arguments and folder, or a
//! remote URL), the tools it may run without asking and the names of the variables or the one secret header it needs.
//! Each computer then sets the program's path and the values. The fields are checked by the rules `add_mcp_server`
//! applies, so a path or a value has nowhere to go.

mod edit;

use lurq::{app::ctx::Ctx, components::Row, core::Signal};
use orchester_core::connection::mcp::McpServerName;
use orchester_workspace::{
  app_mcp::setup::{Header, Invalid, NewTeamMcpServer, Runs, team_mcp_server},
  messages::Message,
};

pub(in crate::screens::team_hub) use self::edit::EditView;
use super::{
  fields::{self, Body, Error, Look, Shown, Spec, name_error},
  work::Make,
};

const NAME: Spec = Spec::new("mcp-name", "team_new.mcp.name")
  .sub("team_new.mcp.nameSub")
  .required();
const RUNS_AS: Spec = Spec::new("mcp-runs-as", "team_new.mcp.runsAs");
const PROGRAM: Spec = Spec::new("mcp-program", "team_new.mcp.program")
  .sub("team_new.mcp.programSub")
  .required();
/// The edit's rows say it shorter (A13 `WMtxm`).
const PROGRAM_EDIT: Spec = Spec::new("mcp-program", "team_new.mcp.program").sub("team_hub.mcp.fromTeam.programSub");
const VARIABLES_EDIT: Spec = Spec::new("mcp-variables", "team_new.mcp.variables").sub("team_hub.edit.variablesSub");
const ARGUMENTS: Spec = Spec::new("mcp-arguments", "team_new.mcp.arguments").sub("team_new.mcp.argumentsSub");
const FOLDER: Spec = Spec::new("mcp-folder", "team_new.mcp.folder");
const FOLDER_PATH: Spec = Spec::new("mcp-folder-path", "team_new.mcp.folderPath").sub("team_new.mcp.folderPathSub");
const EXEMPT: Spec = Spec::new("mcp-exempt", "team_new.mcp.exempt").sub("team_new.mcp.exemptSub");
const VARIABLES: Spec = Spec::new("mcp-variables", "team_new.mcp.variables").sub("team_new.mcp.variablesSub");
const URL: Spec = Spec::new("mcp-url", "team_new.mcp.url")
  .sub("team_new.mcp.urlSub")
  .required();
const HEADER: Spec = Spec::new("mcp-header", "team_new.mcp.header").sub("team_new.mcp.headerSub");
const SECRET: Spec = Spec::new("mcp-secret", "team_new.mcp.secret").sub("team_new.mcp.secretSub");
const PREFIX: Spec = Spec::new("mcp-prefix", "team_new.mcp.prefix").sub("team_new.mcp.prefixSub");

/// A server starts in the run's worktree, or in a folder inside it: the team body holds no other folder.
const WORKTREE: &str = "worktree";
const INSIDE: &str = "inside";

const PROGRAM_RUN: &str = "program";
const RUNS: [(&str, &str); 2] = [
  (PROGRAM_RUN, "team_new.mcp.runsProgram"),
  ("remote", "team_new.mcp.runsRemote"),
];

#[derive(Clone)]
pub(in crate::screens::team_hub) struct Fields {
  name: Signal<String>,
  runs: Signal<String>,
  program: Signal<String>,
  arguments: Signal<String>,
  folder_choice: Signal<String>,
  folder: Signal<String>,
  exempt: Signal<String>,
  variables: Signal<String>,
  url: Signal<String>,
  header: Signal<String>,
  secret: Signal<String>,
  prefix: Signal<String>,
}

/// The tool names a single-line field holds: separated by spaces or commas, none empty.
fn names(text: &str) -> Vec<String> {
  text
    .split(|char: char| char.is_whitespace() || char == ',')
    .filter(|name| !name.is_empty())
    .map(str::to_owned)
    .collect()
}

/// `lines` as a list field holds them, one to a line.
fn lines_text(lines: &[String]) -> String {
  lines.join("\n")
}

/// The lines of a list field, each trimmed, none empty.
fn lines(text: &str) -> Vec<String> {
  text
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty())
    .map(str::to_owned)
    .collect()
}

impl Fields {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      name: ctx.signal(String::new()),
      runs: ctx.signal(PROGRAM_RUN.to_owned()),
      program: ctx.signal(String::new()),
      arguments: ctx.signal(String::new()),
      folder_choice: ctx.signal(WORKTREE.to_owned()),
      folder: ctx.signal(String::new()),
      exempt: ctx.signal(String::new()),
      variables: ctx.signal(String::new()),
      url: ctx.signal(String::new()),
      header: ctx.signal(String::new()),
      secret: ctx.signal(String::new()),
      prefix: ctx.signal(String::new()),
    }
  }

  /// A folder inside the run's worktree is chosen and its path is empty: that is the worktree, which the other choice
  /// says.
  pub(in crate::screens::team_hub) fn folder_missing(&self) -> bool {
    self.runs.get() == PROGRAM_RUN && self.folder_choice.get() == INSIDE && self.folder.get().trim().is_empty()
  }

  /// What the fields describe, as the form of the team server takes it.
  pub(in crate::screens::team_hub) fn input(&self) -> NewTeamMcpServer {
    let runs = if self.runs.get() == PROGRAM_RUN {
      Runs::Program {
        program: self.program.get().trim().to_owned(),
        arguments: lines(&self.arguments.get()),
        folder: if self.folder_choice.get() == INSIDE {
          self.folder.get().trim().to_owned()
        } else {
          String::new()
        },
        variables: lines(&self.variables.get()),
      }
    } else {
      let header = self.header.get().trim().to_owned();
      Runs::Remote {
        url: self.url.get().trim().to_owned(),
        header: (!header.is_empty() || !self.secret.get().trim().is_empty()).then(|| Header {
          name: header,
          secret: self.secret.get().trim().to_owned(),
          prefix: self.prefix.get(),
        }),
      }
    };
    NewTeamMcpServer {
      name: self.name.get().trim().to_owned(),
      runs,
      exempt_tools: names(&self.exempt.get()),
    }
  }

  pub(super) fn body(&self, ctx: &mut Ctx, shown: &Shown<'_>) -> Body {
    let input = self.input();
    let taken: Vec<McpServerName> = shown
      .taken
      .iter()
      .filter_map(|name| McpServerName::new(name).ok())
      .collect();
    let checked = team_mcp_server(&input, &taken);
    let invalid = checked.as_ref().err().map(|invalid| (**invalid).clone());
    let name = self.name.get();
    let program = matches!(input.runs, Runs::Program { .. });
    let mut rows = vec![
      self.name_row(ctx, shown, &name, invalid.as_ref()),
      fields::row(
        ctx,
        &RUNS_AS,
        None,
        fields::choice(ctx, &RUNS_AS, &self.runs, &RUNS, shown.locked),
      ),
    ];
    if program {
      rows.extend(self.program_rows(ctx, shown, invalid.as_ref()));
      rows.push(self.exempt_row(ctx, shown));
      rows.push(self.variables_row(ctx, shown));
    } else {
      rows.extend(self.remote_rows(ctx, shown, invalid.as_ref()));
      rows.push(self.exempt_row(ctx, shown));
    }
    Body {
      rows,
      computer: if program {
        "team_new.computer.mcpProgram"
      } else {
        "team_new.computer.mcpRemote"
      },
      make: checked
        .ok()
        .filter(|_| !self.folder_missing())
        .map(|_| Make::McpServer(Box::new(input))),
      off: fields::off(shown, &name),
      overlay: None,
    }
  }

  fn name_row(&self, ctx: &mut Ctx, shown: &Shown<'_>, name: &str, invalid: Option<&Invalid>) -> Row {
    let says = name_error(ctx, shown, name, "team_new.mcp.nameTaken").or_else(|| match invalid {
      Some(Invalid::Name(message)) if shown.tried => Some(Error::Said(message.clone())),
      _ => None,
    });
    fields::name_row(
      ctx,
      shown,
      &NAME,
      says.as_ref(),
      (&self.name, "team_new.mcp.namePlaceholder"),
      Look::Plain,
    )
  }

  /// What is wrong with a field once Create was pressed: it is empty, or the form was refused for it; the form's own
  /// refusal is said at `main`, the field that says how the server runs.
  fn says(
    shown: &Shown<'_>,
    invalid: Option<&Invalid>,
    (main, empty): (bool, bool),
    own: fn(&Invalid) -> Option<&Message>,
  ) -> Option<Error> {
    if !shown.tried {
      return None;
    }
    if empty {
      return Some(Error::Required);
    }
    let invalid = invalid?;
    match (own(invalid), invalid) {
      (Some(message), _) => Some(Error::Said(message.clone())),
      (None, Invalid::Form(messages)) if main => messages.first().cloned().map(Error::Said),
      _ => None,
    }
  }

  fn program_rows(&self, ctx: &mut Ctx, shown: &Shown<'_>, invalid: Option<&Invalid>) -> Vec<Row> {
    let empty = self.program.get().trim().is_empty();
    let program = Self::says(shown, invalid, (true, empty), |found| match found {
      Invalid::Program(message) => Some(message),
      _ => None,
    });
    let folder = Self::says(shown, invalid, (false, self.folder_missing()), |found| match found {
      Invalid::Folder(message) => Some(message),
      _ => None,
    });
    let marks = Marks::of(&self.input(), shown.published);
    let locked = shown.locked;
    let mut rows = vec![
      fields::text_row(
        ctx,
        if shown.published.is_some() {
          &PROGRAM_EDIT
        } else {
          &PROGRAM
        },
        marks.on(program, marks.program).as_ref(),
        (&self.program, "team_new.mcp.programPlaceholder"),
        (Look::Mono, locked),
      ),
      fields::text_row(
        ctx,
        &ARGUMENTS,
        marks.on(None, marks.arguments).as_ref(),
        (&self.arguments, "team_new.mcp.argumentsPlaceholder"),
        (Look::Lines, locked),
      ),
      self.folder_row(ctx, locked),
    ];
    if self.folder_choice.get() == INSIDE {
      rows.push(fields::text_row(
        ctx,
        &FOLDER_PATH,
        marks.on(folder, marks.folder).as_ref(),
        (&self.folder, "team_new.mcp.folderPathPlaceholder"),
        (Look::Mono, locked),
      ));
    }
    rows
  }

  /// The variables the server needs, one name per line, after the tools it may run without asking (A13).
  fn variables_row(&self, ctx: &mut Ctx, shown: &Shown<'_>) -> Row {
    let marks = Marks::of(&self.input(), shown.published);
    fields::text_row(
      ctx,
      if shown.published.is_some() {
        &VARIABLES_EDIT
      } else {
        &VARIABLES
      },
      marks.on(None, marks.variables).as_ref(),
      (&self.variables, "team_new.mcp.variablesPlaceholder"),
      (Look::Lines, shown.locked),
    )
  }

  /// Where the server starts: in the run's worktree, as the design's select says, or in a folder inside it.
  fn folder_row(&self, ctx: &Ctx, locked: bool) -> Row {
    let options = vec![
      (WORKTREE.to_owned(), ctx.t("team_new.mcp.folderWorktree").to_string()),
      (INSIDE.to_owned(), ctx.t("team_new.mcp.folderInside").to_string()),
    ];
    let select = fields::select(
      ctx,
      &FOLDER,
      &self.folder_choice,
      options,
      ("team_new.mcp.folderWorktree", false, locked),
    );
    fields::row(ctx, &FOLDER, None, select)
  }

  fn remote_rows(&self, ctx: &mut Ctx, shown: &Shown<'_>, invalid: Option<&Invalid>) -> Vec<Row> {
    let empty = self.url.get().trim().is_empty();
    let url = Self::says(shown, invalid, (true, empty), |found| match found {
      Invalid::Url(message) => Some(message),
      _ => None,
    });
    let marks = Marks::of(&self.input(), shown.published);
    let locked = shown.locked;
    vec![
      fields::text_row(
        ctx,
        &URL,
        marks.on(url, marks.url).as_ref(),
        (&self.url, "team_new.mcp.urlPlaceholder"),
        (Look::Mono, locked),
      ),
      fields::text_row(
        ctx,
        &HEADER,
        marks.on(None, marks.header).as_ref(),
        (&self.header, "team_new.mcp.headerPlaceholder"),
        (Look::Mono, locked),
      ),
      fields::text_row(
        ctx,
        &SECRET,
        marks.on(None, marks.secret).as_ref(),
        (&self.secret, "team_new.mcp.secretPlaceholder"),
        (Look::Mono, locked),
      ),
      fields::text_row(
        ctx,
        &PREFIX,
        marks.on(None, marks.prefix).as_ref(),
        (&self.prefix, "team_new.mcp.prefixPlaceholder"),
        (Look::Mono, locked),
      ),
    ]
  }

  fn exempt_row(&self, ctx: &mut Ctx, shown: &Shown<'_>) -> Row {
    let marks = Marks::of(&self.input(), shown.published);
    fields::text_row(
      ctx,
      &EXEMPT,
      marks.on(None, marks.exempt).as_ref(),
      (&self.exempt, "team_new.mcp.exemptPlaceholder"),
      (Look::Mono, shown.locked),
    )
  }
}

/// Which fields of an edit differ from the version it starts from; none for a form that creates.
#[derive(Default)]
struct Marks {
  runs: bool,
  program: bool,
  arguments: bool,
  folder: bool,
  variables: bool,
  url: bool,
  header: bool,
  secret: bool,
  prefix: bool,
  exempt: bool,
}

impl Marks {
  fn of(input: &NewTeamMcpServer, published: Option<&NewTeamMcpServer>) -> Self {
    let Some(published) = published else {
      return Self::default();
    };
    let mut marks = Self {
      exempt: input.exempt_tools != published.exempt_tools,
      ..Self::default()
    };
    match (&input.runs, &published.runs) {
      (
        Runs::Program {
          program,
          arguments,
          folder,
          variables,
        },
        Runs::Program {
          program: was_program,
          arguments: was_arguments,
          folder: was_folder,
          variables: was_variables,
        },
      ) => {
        marks.program = program != was_program;
        marks.arguments = arguments != was_arguments;
        marks.folder = folder != was_folder;
        marks.variables = variables != was_variables;
      }
      (
        Runs::Remote { url, header },
        Runs::Remote {
          url: was_url,
          header: was_header,
        },
      ) => {
        marks.url = url != was_url;
        let (name, secret, prefix) = header.as_ref().map_or((None, None, None), |h| {
          (Some(&h.name), Some(&h.secret), Some(&h.prefix))
        });
        let (was_name, was_secret, was_prefix) = was_header.as_ref().map_or((None, None, None), |h| {
          (Some(&h.name), Some(&h.secret), Some(&h.prefix))
        });
        marks.header = name != was_name;
        marks.secret = secret != was_secret;
        marks.prefix = prefix != was_prefix;
      }
      // Another way of running: every field of the new one is new.
      (Runs::Program { .. }, Runs::Remote { .. }) => {
        marks.runs = true;
        marks.program = true;
        marks.arguments = true;
        marks.folder = true;
        marks.variables = true;
      }
      (Runs::Remote { .. }, Runs::Program { .. }) => {
        marks.runs = true;
        marks.url = true;
        marks.header = true;
        marks.secret = true;
        marks.prefix = true;
      }
    }
    marks
  }

  /// The labels (keys) of the changed fields, in the order of the rows.
  fn labels(&self) -> Vec<&'static str> {
    [
      (self.runs, RUNS_AS.label),
      (self.program, PROGRAM.label),
      (self.arguments, ARGUMENTS.label),
      (self.folder, FOLDER_PATH.label),
      (self.variables, VARIABLES.label),
      (self.url, URL.label),
      (self.header, HEADER.label),
      (self.secret, SECRET.label),
      (self.prefix, PREFIX.label),
      (self.exempt, EXEMPT.label),
    ]
    .into_iter()
    .filter_map(|(changed, label)| changed.then_some(label))
    .collect()
  }

  /// `error` of a field, else *changed in your draft* when `changed`.
  fn on(&self, error: Option<Error>, changed: bool) -> Option<Error> {
    error.or_else(|| changed.then_some(Error::Changed))
  }
}
