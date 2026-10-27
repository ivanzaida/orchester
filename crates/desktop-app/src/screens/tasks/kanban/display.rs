//! How the Tasks screen shows a project's tasks, kept per project in the application database's window state
//! (`ui_state`, key `orchester-app.tasks.display.<project>`), after a restart too (`What it adds` `bNMv9`, *Board
//! layout*): the List or the Board (`Layout` in `Header`, `09 Display options` `J0ncJ`), what the board's columns and
//! rows group by, which card properties show, whether empty columns and the Canceled column show, and which columns and
//! rows are folded away. *Reset to default* brings back the board's defaults; the layout stays.

use orchester_core::id::ProjectId;
use orchester_workspace::workspace::Workspace;
use serde_json::{Value, json};

/// The list's rows or the board's columns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layout {
  #[default]
  List,
  Board,
}

/// What the board's columns are (`Columns menu` `xK96w`): the workflow statuses, or the priorities, the modules or the
/// pipelines. Columns by pipeline only show: a pipeline is chosen on the task's page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Columns {
  #[default]
  Status,
  Priority,
  Module,
  Pipeline,
}

impl Columns {
  pub const ALL: [Self; 4] = [Self::Status, Self::Priority, Self::Module, Self::Pipeline];

  pub const fn name(self) -> &'static str {
    match self {
      Self::Status => "status",
      Self::Priority => "priority",
      Self::Module => "module",
      Self::Pipeline => "pipeline",
    }
  }

  fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|columns| columns.name() == name)
  }
}

/// What the board's rows (swimlanes, `10 Swimlanes` `i96kI`) group by: nothing, the modules or the priorities.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lanes {
  #[default]
  None,
  Module,
  Priority,
}

impl Lanes {
  pub const ALL: [Self; 3] = [Self::None, Self::Module, Self::Priority];

  pub const fn name(self) -> &'static str {
    match self {
      Self::None => "none",
      Self::Module => "module",
      Self::Priority => "priority",
    }
  }

  fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|lanes| lanes.name() == name)
  }
}

/// A part of a card that Display options turn on or off (`Card properties` in `kQPuZ`). The title always shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Property {
  Key,
  Priority,
  Points,
  Module,
  Tags,
  Run,
  Readiness,
  Agent,
  Age,
  Blocked,
  Pipeline,
}

impl Property {
  pub const ALL: [Self; 11] = [
    Self::Key,
    Self::Priority,
    Self::Points,
    Self::Module,
    Self::Tags,
    Self::Run,
    Self::Readiness,
    Self::Agent,
    Self::Age,
    Self::Blocked,
    Self::Pipeline,
  ];

  pub const fn name(self) -> &'static str {
    match self {
      Self::Key => "key",
      Self::Priority => "priority",
      Self::Points => "points",
      Self::Module => "module",
      Self::Tags => "tags",
      Self::Run => "run",
      Self::Readiness => "readiness",
      Self::Agent => "agent",
      Self::Age => "age",
      Self::Blocked => "blocked",
      Self::Pipeline => "pipeline",
    }
  }

  fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|property| property.name() == name)
  }
}

/// How a project's tasks are shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Display {
  pub layout: Layout,
  pub columns: Columns,
  pub lanes: Lanes,
  /// The card properties turned off; every other shows.
  pub hidden: Vec<Property>,
  pub show_empty: bool,
  pub show_canceled: bool,
  /// The columns folded to a strip, by their keys.
  pub collapsed: Vec<String>,
  /// The rows folded to their header, by their keys.
  pub folded: Vec<String>,
}

impl lurq::app::component::DevtoolsInspectable for Display {}

impl Default for Display {
  /// The list; on the board, columns by status without rows, every property but the pipeline, empty columns shown and
  /// Canceled hidden.
  fn default() -> Self {
    Self {
      layout: Layout::List,
      columns: Columns::Status,
      lanes: Lanes::None,
      hidden: vec![Property::Pipeline],
      show_empty: true,
      show_canceled: false,
      collapsed: Vec::new(),
      folded: Vec::new(),
    }
  }
}

impl Display {
  pub fn shows(&self, property: Property) -> bool {
    !self.hidden.contains(&property)
  }

  /// The board's defaults, the layout kept.
  #[must_use]
  pub fn reset(&self) -> Self {
    Self {
      layout: self.layout,
      ..Self::default()
    }
  }

  /// `property` shown, or hidden when it shows.
  pub fn toggle(&mut self, property: Property) {
    toggle(&mut self.hidden, property);
  }

  /// The column `key` folded to a strip, or unfolded when it is.
  pub fn toggle_collapsed(&mut self, key: &str) {
    toggle(&mut self.collapsed, key.to_owned());
  }

  /// The row `key` folded to its header, or unfolded when it is.
  pub fn toggle_folded(&mut self, key: &str) {
    toggle(&mut self.folded, key.to_owned());
  }

  fn encode(&self) -> String {
    json!({
      "layout": match self.layout { Layout::List => "list", Layout::Board => "board" },
      "columns": self.columns.name(),
      "lanes": self.lanes.name(),
      "hidden": self.hidden.iter().map(|property| property.name()).collect::<Vec<_>>(),
      "showEmpty": self.show_empty,
      "showCanceled": self.show_canceled,
      "collapsed": self.collapsed,
      "folded": self.folded,
    })
    .to_string()
  }

  /// What `text` keeps; a part that cannot be read keeps its default.
  fn decode(text: &str) -> Option<Self> {
    let value: Value = serde_json::from_str(text).ok()?;
    let default = Self::default();
    let names = |field: &str| -> Option<Vec<String>> {
      Some(
        value
          .get(field)?
          .as_array()?
          .iter()
          .filter_map(|name| Some(name.as_str()?.to_owned()))
          .collect(),
      )
    };
    Some(Self {
      layout: match value.get("layout").and_then(Value::as_str) {
        Some("board") => Layout::Board,
        _ => Layout::List,
      },
      columns: value
        .get("columns")
        .and_then(Value::as_str)
        .and_then(Columns::from_name)
        .unwrap_or(default.columns),
      lanes: value
        .get("lanes")
        .and_then(Value::as_str)
        .and_then(Lanes::from_name)
        .unwrap_or(default.lanes),
      hidden: names("hidden").map_or(default.hidden, |names| {
        names.iter().filter_map(|name| Property::from_name(name)).collect()
      }),
      show_empty: value
        .get("showEmpty")
        .and_then(Value::as_bool)
        .unwrap_or(default.show_empty),
      show_canceled: value
        .get("showCanceled")
        .and_then(Value::as_bool)
        .unwrap_or(default.show_canceled),
      collapsed: names("collapsed").unwrap_or_default(),
      folded: names("folded").unwrap_or_default(),
    })
  }
}

fn toggle<T: PartialEq>(values: &mut Vec<T>, value: T) {
  match values.iter().position(|held| *held == value) {
    Some(index) => {
      values.remove(index);
    }
    None => values.push(value),
  }
}

fn key(project: ProjectId) -> String {
  format!("orchester-app.tasks.display.{project}")
}

/// How `project`'s tasks were last shown; the defaults when nothing was kept. Blocks on SQLite (one row). What cannot
/// be read is logged and reads as the defaults: only this app writes it.
pub fn read(workspace: &Workspace, project: ProjectId) -> Display {
  match workspace.ui_state(&key(project)) {
    Ok(Some(text)) => Display::decode(&text).unwrap_or_else(|| {
      tracing::warn!("the Tasks display kept for the project cannot be read; using its defaults");
      Display::default()
    }),
    Ok(None) => Display::default(),
    Err(error) => {
      tracing::warn!(%error, "the Tasks display kept for the project cannot be read");
      Display::default()
    }
  }
}

/// Keeps `display` for `project`. Blocks on SQLite; a failure only loses the choice after a restart, so it is logged.
pub fn keep(workspace: &Workspace, project: ProjectId, display: &Display) {
  if let Err(error) = workspace.set_ui_state(&key(project), &display.encode()) {
    tracing::warn!(%error, "the Tasks display is not kept for the project");
  }
}
