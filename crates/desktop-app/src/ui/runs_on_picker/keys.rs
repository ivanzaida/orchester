//! The compact *Runs on* picker's keys, as pure state. The picker has three areas, the rail, the models and the
//! effort row; Tab and Shift+Tab move the keys between the ones that have something to move over (round, in that
//! order), ← and → move within the rail and within the effort row, ↑ and ↓ within the models, and Enter (Space too on
//! an effort) chooses where the keys are (a connection that cannot be chosen is refused and the picker says why). An
//! arrow that belongs to another area takes the keys there: ← and → from the models to the rail, ↑ and ↓ from the rail
//! or the efforts to the models. The first press of an arrow in the list shows where the keys start: the chosen model,
//! else the first; while Search holds text, the first model found already has them.

/// Where the keys are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
  Rail,
  List,
  Effort,
}

/// The areas in the order Tab goes through them: the order they are drawn in.
const AREAS: [Place; 3] = [Place::Rail, Place::List, Place::Effort];

impl Place {
  const fn order(self) -> usize {
    match self {
      Self::Rail => 0,
      Self::List => 1,
      Self::Effort => 2,
    }
  }
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
  /// Chooses on the effort row only; anywhere else it is typed into Search.
  Space,
  /// Tab: on to the next area.
  Next,
  /// Shift+Tab: back to the previous area.
  Previous,
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
  /// How many segments the effort row offers (*Default* among them), 0 while it is inert, and where the chosen one is.
  pub efforts: usize,
  pub effort: Option<usize>,
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
  /// The segment at this place in the effort row is chosen.
  Effort(usize),
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
      Place::Effort => shape.efforts,
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
      Key::Left => self.sideways(-1, shape),
      Key::Right => self.sideways(1, shape),
      Key::Up => self.in_list(-1, shape),
      Key::Down => self.in_list(1, shape),
      Key::Enter => self.choose(shape),
      Key::Space if self.place == Place::Effort => self.choose(shape),
      Key::Space => Outcome::Ignored,
      Key::Next => self.between(1, shape),
      Key::Previous => self.between(-1, shape),
    }
  }

  /// ← and →: along the effort row where the keys are in it, else along the rail.
  fn sideways(&mut self, by: isize, shape: &Shape) -> Outcome {
    match self.place {
      Place::Effort => self.along_efforts(by, shape),
      Place::Rail | Place::List => self.along_rail(by, shape),
    }
  }

  /// Tab: the keys go to the next area in `by`'s direction that has something to move over, at the item they start on
  /// there: the connection shown, the chosen model, the chosen effort (else the first of each).
  fn between(&mut self, by: isize, shape: &Shape) -> Outcome {
    let from = self.place.order();
    for step in 1..=AREAS.len() {
      // Three areas: the casts are exact.
      let to = (from as isize + by * step as isize).rem_euclid(AREAS.len() as isize) as usize;
      let place = AREAS[to];
      if let Some(at) = Self::entry(place, shape) {
        *self = Self { place, at: Some(at) };
        return Outcome::Moved;
      }
    }
    Outcome::Ignored
  }

  /// Where the keys start on entering `place`; `None` when it has nothing to move over.
  fn entry(place: Place, shape: &Shape) -> Option<usize> {
    let (count, start) = match place {
      Place::Rail => (shape.rail.len(), shape.viewing),
      Place::List => (shape.list, shape.chosen),
      Place::Effort => (shape.efforts, shape.effort),
    };
    (count > 0).then(|| start.unwrap_or(0).min(count - 1))
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

  fn along_efforts(&mut self, by: isize, shape: &Shape) -> Outcome {
    if shape.efforts == 0 {
      return Outcome::Ignored;
    }
    let from = self.shown(Place::Effort, shape).or(shape.effort).unwrap_or(0);
    let next = from.saturating_add_signed(by).min(shape.efforts - 1);
    *self = Self {
      place: Place::Effort,
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
      Place::Effort => Outcome::Effort(at),
    }
  }
}

impl lurq::app::component::DevtoolsInspectable for Cursor {}
