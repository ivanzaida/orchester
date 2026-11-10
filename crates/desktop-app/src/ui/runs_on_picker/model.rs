//! What the compact *Runs on* picker is given and hands back: plain data with no app type in it, so any screen can
//! open the picker over what it offers.

use std::sync::Arc;

use crate::ui::{agent_mark::AgentMark, icon::Icon};

/// How many models the list shows before the rest scroll (`BYfVi` in `ZcGGy`: 4 rows of 28 in 4 of padding).
pub const LIST_ROWS: usize = 4;

/// The mark a connection has on the rail; a divider falls between two neighbours whose marks differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
  Agent(AgentMark),
  /// An API provider's cloud.
  Cloud,
}

/// A model of a connection, and the efforts it offers (as the CLI names them: `low`, `medium`, `high`, `xhigh`,
/// `max`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
  pub id: Arc<str>,
  pub efforts: Vec<Arc<str>>,
}

/// What a connection's line says on a second line under its name, in Geist Mono (`Line 2` `B3gJPd`): an API
/// provider's tokens today, a plan's limit reached. It takes the place of the 5-hour use before the meter, which then
/// stands at the end of that line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Detail {
  pub text: Arc<str>,
  /// What its tooltip says, when it has one.
  pub tip: Option<Arc<str>>,
  /// The line is in `warning`: the caption above it too, and the meter.
  pub warns: bool,
}

/// How a note under the efforts speaks (`Data note` `v1AfDF`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeTone {
  /// Where data goes: on `warning_muted` with a `warning` glyph.
  Warning,
  /// A refusal: on `surface_input` with a `danger` glyph.
  Danger,
  /// Anything else the person should know: on `surface_input`.
  Neutral,
}

/// A note at the compact picker's foot: a 12 glyph and its words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
  pub icon: Icon,
  pub tone: NoticeTone,
  pub text: Arc<str>,
}

/// A connection on the rail.
#[derive(Clone, Debug, PartialEq)]
pub struct Connection {
  pub id: Arc<str>,
  pub mark: Mark,
  /// Its name on the line under the rail (*Work*).
  pub name: Arc<str>,
  /// What follows its name there (*Claude Code · Max plan*).
  pub caption: Arc<str>,
  /// What its tooltip says (*Claude Code · Work*).
  pub tip: Arc<str>,
  /// How much of the plan's 5-hour window is used, when the plan is read.
  pub five_hour: Option<u8>,
  /// The plan is at its limit: its item takes the warning dot.
  pub limit: bool,
  /// Why it cannot be chosen, when it cannot: its item is at 40 % and its tooltip says this.
  pub unavailable: Option<Arc<str>>,
  /// A second line under its name, and whether it warns.
  pub detail: Option<Detail>,
  /// What its tooltip says under its name (*Another account starts a new session…*).
  pub tip_note: Option<Arc<str>>,
  /// Search takes a model of the person's own (*Search or type a model…*): what the connection lists is not all it
  /// takes.
  pub typed_model: bool,
  /// It lists no efforts: the effort is typed in a field, and empty is *Default*.
  pub typed_effort: bool,
  /// A note under its efforts while it is shown (where its data goes).
  pub notice: Option<Notice>,
  pub models: Vec<Model>,
}

/// What is chosen now: the connection, and on it a model and an effort (`None`: the CLI's own).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chosen {
  pub connection: Arc<str>,
  pub model: Option<Arc<str>>,
  pub effort: Option<Arc<str>>,
}

/// What the person chose; the picker applies nothing itself, its caller does and passes what is chosen back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
  Connection(Arc<str>),
  Model {
    connection: Arc<str>,
    model: Arc<str>,
  },
  /// The chosen model's effort; `None` is *Default*.
  Effort(Option<Arc<str>>),
}

/// The models whose id holds `query` (case does not matter), in their order.
pub fn filter_models<'a>(models: &'a [Model], query: &str) -> Vec<&'a Model> {
  let wanted = query.trim().to_lowercase();
  models
    .iter()
    .filter(|model| model.id.to_lowercase().contains(&wanted))
    .collect()
}

impl lurq::app::component::DevtoolsInspectable for Chosen {}
