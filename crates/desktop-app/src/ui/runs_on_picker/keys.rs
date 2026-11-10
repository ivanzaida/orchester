//! The compact *Runs on* picker's keys, as pure state: ← and → move along the rail, ↑ and ↓ in the list, Enter chooses
//! where the keys are (a connection that cannot be chosen is refused and the picker says why). The first press of an
//! arrow in the list shows where the keys start: the chosen model, else the first; while Search holds text, the first
//! model found already has them.

/// Where the keys are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
  Rail,
  List,
}

impl lurq::app::component::DevtoolsInspectable for Place {}

/// A key the picker takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
  Left,
  Right,
  Up,
  Down,
  Enter,
}

/// What the keys move over now.
#[derive(Clone, Copy, Debug)]
pub struct Shape<'a> {
  /// Whether each connection on the rail can be chosen.
  pub rail: &'a [bool],
  /// The connection shown, on the rail.
  pub viewing: Option<usize>,
  /// How many models the list holds as Search filters it, and where the chosen one is.
  pub list: usize,
  pub chosen: Option<usize>,
  /// Search holds text.
  pub searched: bool,
}

/// What a key did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
  /// Nothing to do with the key.
  Ignored,
  /// The keys moved.
  Moved,
  /// The connection at this place on the rail is chosen.
  Connection(usize),
  /// The connection at this place on the rail cannot be chosen.
  Refused(usize),
  /// The model at this place in the list is chosen.
  Model(usize),
}

/// The keys' place, and where in it once an arrow moved them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
  pub place: Place,
  pub at: Option<usize>,
}

impl Cursor {
  pub const START: Self = Self {
    place: Place::List,
    at: None,
  };

  /// Where the keys are in `place`: where an arrow moved them, else the first model found while Search holds text.
  pub fn shown(self, place: Place, shape: &Shape) -> Option<usize> {
    if self.place != place {
      return None;
    }
    let at = self.at.or((place == Place::List && shape.searched).then_some(0))?;
    let count = match place {
      Place::Rail => shape.rail.len(),
      Place::List => shape.list,
    };
    (count > 0).then(|| at.min(count - 1))
  }

  /// What was typed in Search: the keys go to the models it finds, from the first.
  pub fn typed(&mut self) {
    *self = Self::START;
  }

  /// `key` pressed with the keys at `self` over `shape`.
  pub fn press(&mut self, key: Key, shape: &Shape) -> Outcome {
    match key {
      Key::Left => self.along_rail(-1, shape),
      Key::Right => self.along_rail(1, shape),
      Key::Up => self.in_list(-1, shape),
      Key::Down => self.in_list(1, shape),
      Key::Enter => self.choose(shape),
    }
  }

  fn along_rail(&mut self, by: isize, shape: &Shape) -> Outcome {
    let count = shape.rail.len();
    if count == 0 {
      return Outcome::Ignored;
    }
    let from = self.shown(Place::Rail, shape).or(shape.viewing).unwrap_or(0);
    let next = from.saturating_add_signed(by).min(count - 1);
    *self = Self {
      place: Place::Rail,
      at: Some(next),
    };
    Outcome::Moved
  }

  fn in_list(&mut self, by: isize, shape: &Shape) -> Outcome {
    if shape.list == 0 {
      return Outcome::Ignored;
    }
    let next = match self.shown(Place::List, shape) {
      Some(at) => at.saturating_add_signed(by).min(shape.list - 1),
      None => shape.chosen.unwrap_or(0).min(shape.list - 1),
    };
    *self = Self {
      place: Place::List,
      at: Some(next),
    };
    Outcome::Moved
  }

  fn choose(&self, shape: &Shape) -> Outcome {
    let Some(at) = self.shown(self.place, shape) else {
      return Outcome::Ignored;
    };
    match self.place {
      Place::Rail if shape.rail[at] => Outcome::Connection(at),
      Place::Rail => Outcome::Refused(at),
      Place::List => Outcome::Model(at),
    }
  }
}

impl lurq::app::component::DevtoolsInspectable for Cursor {}
