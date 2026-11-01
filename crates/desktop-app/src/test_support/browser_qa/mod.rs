//! The Browser window's QA fixture (ORC-379): which state of `Browser · redesign` `lcNez` a QA copy plays into the
//! window's state, for which project and runs, written by the seed beside the app's database ([`Fixture::FILE`]) and
//! played by the QA app ([`play`]) as a project's runner would report its browser: tabs with their owners, the tab
//! shown, frames of a page, what an agent did, a dialog, a page's request for a file or a `<select>` list. Fixture
//! data for QA, never shipped: only the `test-support` feature builds it.

mod play;
mod tabs;

use std::{
  fs, io,
  path::{Path, PathBuf},
};

use orchester_core::id::RunId;
use serde_json::{Value, json};

pub use self::play::{Page, play};

/// A state of the board, by the frame it reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserState {
  /// `01 · Default` `uCSan`: agents' tabs, one new and one acting, then the person's.
  Default,
  /// `01c · Many runs` `fPkxD`: tabs crowded to their narrowest.
  Many,
  /// `01d · Tab overflow` `fWhIr`, `01e` `cmofl`: twenty tabs.
  Overflow,
  /// `02 · Agent is acting` `UKAkY`: the agent of the tab shown types into a field.
  Acting,
  /// `02b · You use the tab` `k7xvx`: the agent of the tab shown waits because the person uses it, as a runner reports
  /// it again and again while the person goes on.
  Waiting,
  /// `03` `t08Ur`: a confirm on the person's own tab.
  Dialog,
  /// `03b · File chooser · dialog open` `TWpyp`: the page's *Signing certificate* field asks for a file on the person's
  /// own tab, and the window's hint stays over the page, as the QA copy's stand-in dialog never closes.
  FileChooser,
  /// `03c · File chooser · file chosen` `x0N8d`: the field shows the file chosen in it.
  FileChosen,
  /// `04` `r31HX`: a `<select>` list open on the person's own tab.
  Select,
  /// `05` `GjcAk`: the browser in a window of its own.
  Headed,
  /// `06 · Empty` `eCCuU`: no tabs.
  Empty,
  /// `07 · Browser not found` `YEv2c`: the session finds no browser.
  NotFound,
}

impl BrowserState {
  const ALL: [Self; 12] = [
    Self::Default,
    Self::Many,
    Self::Overflow,
    Self::Acting,
    Self::Waiting,
    Self::Dialog,
    Self::FileChooser,
    Self::FileChosen,
    Self::Select,
    Self::Headed,
    Self::Empty,
    Self::NotFound,
  ];

  pub const fn name(self) -> &'static str {
    match self {
      Self::Default => "default",
      Self::Many => "many",
      Self::Overflow => "overflow",
      Self::Acting => "acting",
      Self::Waiting => "waiting",
      Self::Dialog => "dialog",
      Self::FileChooser => "file-chooser",
      Self::FileChosen => "file-chosen",
      Self::Select => "select",
      Self::Headed => "headed",
      Self::Empty => "empty",
      Self::NotFound => "not-found",
    }
  }
}

/// What the QA app plays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fixture {
  pub state: BrowserState,
  /// The project's folder, as its runner would report it.
  pub project: PathBuf,
  pub name: String,
  /// The runs whose agents own tabs, the latest of each task that has runs.
  pub runs: Vec<RunId>,
}

impl Fixture {
  /// The fixture's file, beside the app's database.
  pub const FILE: &'static str = "browser-qa.json";

  pub fn to_json(&self) -> Value {
    json!({
      "state": self.state.name(),
      "project": self.project,
      "name": self.name,
      "runs": self.runs.iter().map(ToString::to_string).collect::<Vec<_>>(),
    })
  }

  /// The fixture beside the database at `database`; `None` when there is none.
  pub fn beside(database: &Path) -> Result<Option<Self>, String> {
    let path = database.with_file_name(Self::FILE);
    let text = match fs::read_to_string(&path) {
      Ok(text) => text,
      Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
      Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let value: Value = serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    let field = |name: &str| {
      value
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("no `{name}` in the fixture"))
    };
    let state = field("state")?;
    let state = BrowserState::ALL
      .into_iter()
      .find(|known| known.name() == state)
      .ok_or_else(|| format!("no browser state `{state}`"))?;
    let runs = value
      .get("runs")
      .and_then(Value::as_array)
      .ok_or("no `runs` in the fixture")?
      .iter()
      .map(|run| {
        run
          .as_str()
          .and_then(|run| run.parse().ok())
          .ok_or("a run that is not a run id")
      })
      .collect::<Result<Vec<RunId>, _>>()?;
    Ok(Some(Self {
      state,
      project: PathBuf::from(field("project")?),
      name: field("name")?.to_owned(),
      runs,
    }))
  }
}
