//! Where the pipeline canvas (03.01 `Q21HOA`, 03.B `UjHDL`) draws steps and connectors.
//!
//! Steps are grouped by dependency level. A level with one step that runs right after the
//! previous card continues that card's row to the right (Research → Plan → Implement); a level
//! with several steps gets a row of its own, side by side (Run checks ∥ Review), and the next
//! row starts under the mean of its parents. Connectors go right between cards of a row and
//! bend halfway between rows. A dependency that closes a cycle is drawn around the graph.
//!
//! Positions are the design's canvas coordinates; the constants carry the node ids.

use std::collections::BTreeMap;

use orchester_core::resource::{Step, StepKey};

/// Left edge of the graph and top of the first row (`b6SADv` at 40, 100).
pub const ORIGIN_X: f32 = 40.0;
pub const FIRST_ROW_Y: f32 = 100.0;
/// Step card size (`ajikA`).
pub const CARD_WIDTH: f32 = 224.0;
pub const CARD_HEIGHT: f32 = 116.0;
/// Gap between cards of one row (`zpizf`), and between two parallel cards (`nQinZ` to `skGHv`).
const ROW_GAP_X: f32 = 72.0;
const PARALLEL_GAP_X: f32 = 224.0;
/// Right edge a row may reach before the next step starts a new row (`i1D0tH`).
const ROW_RIGHT_LIMIT: f32 = 856.0;
/// Gap between rows (`K6Eg1`); a parallel row gets more room for its label (`y6esu7`).
const ROW_GAP_Y: f32 = 128.0;
const PARALLEL_ROW_EXTRA: f32 = 20.0;
/// A connector bends this far above the card it leads to, further above a parallel row.
const BEND_ABOVE: f32 = 64.0;
const PARALLEL_BEND_EXTRA: f32 = 16.0;
/// Labels above a parallel row (`qwFek`) and a join (`K7dDI`).
const PARALLEL_LABEL_ABOVE: f32 = 34.0;
const JOIN_LABEL_ABOVE: f32 = 45.0;
const JOIN_LABEL_LEFT: f32 = 120.0;
/// Direction chevrons (`Dr0oM`, `j5Dpx`).
pub const CHEVRON_SIZE: f32 = 14.0;
/// The cycle connector (`gXNw9`) passes this far right of the graph and above the step
/// that waits; its label (`RAoRx`) sits above it.
const CYCLE_RIGHT: f32 = 32.0;
const CYCLE_ABOVE: f32 = 20.0;
const CYCLE_LABEL_ABOVE: f32 = 38.0;
const CYCLE_LABEL_RIGHT: f32 = 152.0;
/// Room under the graph before the failure route notes (`pFKSd`), and around the content.
pub const NOTES_BELOW: f32 = 22.0;
pub const CONTENT_MARGIN: f32 = 40.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardBox {
  pub index: usize,
  pub x: f32,
  pub y: f32,
}

impl CardBox {
  fn center_x(self) -> f32 {
    self.x + CARD_WIDTH / 2.0
  }

  fn right(self) -> f32 {
    self.x + CARD_WIDTH
  }

  fn bottom(self) -> f32 {
    self.y + CARD_HEIGHT
  }

  fn middle_y(self) -> f32 {
    self.y + CARD_HEIGHT / 2.0
  }
}

/// A straight piece of connector from `(x1, y1)` to `(x2, y2)`, horizontal or vertical.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
  pub x1: f32,
  pub y1: f32,
  pub x2: f32,
  pub y2: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
  Right,
  Down,
}

/// A chevron at its top-left corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chevron {
  pub x: f32,
  pub y: f32,
  pub direction: Direction,
}

/// The dependency that closes a cycle, drawn around the graph.
#[derive(Clone, Debug, PartialEq)]
pub struct CycleEdge {
  pub waiting: usize,
  pub dependency: usize,
  pub segments: Vec<Segment>,
  pub chevron: Chevron,
  pub label: (f32, f32),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GraphLayout {
  pub cards: Vec<CardBox>,
  pub segments: Vec<Segment>,
  pub chevrons: Vec<Chevron>,
  /// Top-left of each "run in parallel" label.
  pub parallel_labels: Vec<(f32, f32)>,
  /// Top-left of each join label and how many steps must pass.
  pub join_labels: Vec<(f32, f32, usize)>,
  pub cycle: Option<CycleEdge>,
  /// Right and bottom edges of everything drawn.
  pub right: f32,
  pub bottom: f32,
}

impl GraphLayout {
  pub fn card(&self, index: usize) -> Option<CardBox> {
    self.cards.iter().copied().find(|card| card.index == index)
  }
}

/// Step positions grouped by dependency level, each level in list order. Dependencies on
/// unknown steps are ignored, and so is `closing` — the dependency `(waiting, dependency)`
/// that closes a cycle — so a pipeline being edited into a broken state still has a layout.
pub fn levels_ignoring(steps: &[Step], closing: Option<(usize, usize)>) -> Vec<Vec<usize>> {
  let dependencies = dependencies(steps, closing);
  let mut level: Vec<Option<usize>> = vec![None; steps.len()];
  let mut visiting = vec![false; steps.len()];
  for position in 0..steps.len() {
    depth(position, &dependencies, &mut level, &mut visiting);
  }
  let mut grouped: Vec<Vec<usize>> = Vec::new();
  for (position, found) in level.into_iter().enumerate() {
    let found = found.unwrap_or(0);
    if grouped.len() <= found {
      grouped.resize(found + 1, Vec::new());
    }
    grouped[found].push(position);
  }
  grouped.retain(|group| !group.is_empty());
  grouped
}

/// Positions of each step's dependencies, without unknown steps and the `closing` one.
fn dependencies(steps: &[Step], closing: Option<(usize, usize)>) -> Vec<Vec<usize>> {
  let index: BTreeMap<&StepKey, usize> = steps.iter().enumerate().map(|(i, step)| (&step.key, i)).collect();
  steps
    .iter()
    .enumerate()
    .map(|(position, step)| {
      step
        .depends_on
        .iter()
        .filter_map(|key| index.get(key).copied())
        .filter(|&dependency| closing != Some((position, dependency)))
        .collect()
    })
    .collect()
}

/// [`levels_ignoring`] for a valid pipeline.
pub fn levels(steps: &[Step]) -> Vec<Vec<usize>> {
  levels_ignoring(steps, None)
}

/// Longest chain of dependencies above `position`; an edge back into the chain being walked
/// (a cycle other than the ignored one) counts as no dependency.
fn depth(position: usize, dependencies: &[Vec<usize>], level: &mut [Option<usize>], visiting: &mut [bool]) -> usize {
  if let Some(found) = level[position] {
    return found;
  }
  if visiting[position] {
    return 0;
  }
  visiting[position] = true;
  let mut found = 0;
  for &dependency in &dependencies[position] {
    if !visiting[dependency] {
      found = found.max(depth(dependency, dependencies, level, visiting) + 1);
    }
  }
  visiting[position] = false;
  level[position] = Some(found);
  found
}

/// Lays out `steps`; `closing` is the dependency that closes a cycle, if any.
pub fn layout(steps: &[Step], closing: Option<(usize, usize)>) -> GraphLayout {
  let dependencies = dependencies(steps, closing);
  let mut graph = GraphLayout::default();
  let mut row_top = FIRST_ROW_Y;
  let mut previous: Option<CardBox> = None;
  let mut row_is_sequential = false;
  for level in levels_ignoring(steps, closing) {
    let joins_row = row_is_sequential
      && level.len() == 1
      && previous.is_some_and(|card| {
        card.right() + ROW_GAP_X + CARD_WIDTH <= ROW_RIGHT_LIMIT && dependencies[level[0]] == [card.index]
      });
    if joins_row && let Some(card) = previous {
      let placed = CardBox {
        index: level[0],
        x: card.right() + ROW_GAP_X,
        y: card.y,
      };
      graph.cards.push(placed);
      previous = Some(placed);
      continue;
    }
    if !graph.cards.is_empty() {
      row_top = graph.cards.iter().map(|card| card.bottom()).fold(row_top, f32::max) + ROW_GAP_Y;
      if level.len() > 1 {
        row_top += PARALLEL_ROW_EXTRA;
      }
    }
    let placed = place_row(&graph, &dependencies, &level, row_top);
    previous = placed.last().copied();
    row_is_sequential = placed.len() == 1;
    if placed.len() > 1 {
      graph
        .parallel_labels
        .push((placed[0].x, row_top - PARALLEL_LABEL_ABOVE));
    }
    graph.cards.extend(placed);
  }
  connect(&mut graph, &dependencies, closing);
  graph
}

/// Cards of a new row at `top`. One card goes under the mean of its parents; several go side by
/// side with the last one under the rightmost parent.
fn place_row(graph: &GraphLayout, dependencies: &[Vec<usize>], level: &[usize], top: f32) -> Vec<CardBox> {
  let parents = |position: usize| -> Vec<CardBox> {
    graph
      .cards
      .iter()
      .copied()
      .filter(|card| dependencies[position].contains(&card.index))
      .collect()
  };
  if let [single] = level {
    let above = parents(*single);
    let x = if above.is_empty() {
      ORIGIN_X
    } else {
      above.iter().map(|card| card.center_x()).sum::<f32>() / above.len() as f32 - CARD_WIDTH / 2.0
    };
    return vec![CardBox {
      index: *single,
      x: x.max(ORIGIN_X),
      y: top,
    }];
  }
  let gap = if level.len() == 2 { PARALLEL_GAP_X } else { ROW_GAP_X };
  let step_x = CARD_WIDTH + gap;
  let rightmost_parent = level
    .iter()
    .flat_map(|&position| parents(position))
    .map(|card| card.x)
    .fold(ORIGIN_X, f32::max);
  let first = (rightmost_parent - step_x * (level.len() - 1) as f32).max(ORIGIN_X);
  level
    .iter()
    .enumerate()
    .map(|(slot, &index)| CardBox {
      index,
      x: first + step_x * slot as f32,
      y: top,
    })
    .collect()
}

fn connect(graph: &mut GraphLayout, dependencies: &[Vec<usize>], closing: Option<(usize, usize)>) {
  let cards = graph.cards.clone();
  for child in &cards {
    let parents: Vec<CardBox> = cards
      .iter()
      .copied()
      .filter(|card| dependencies[child.index].contains(&card.index))
      .collect();
    for parent in &parents {
      if (parent.y - child.y).abs() < f32::EPSILON {
        connect_in_row(graph, *parent, *child);
      } else {
        connect_rows(graph, *parent, *child);
      }
    }
    let below_rows = parents.iter().filter(|parent| parent.y < child.y).count();
    if below_rows > 0 {
      graph.chevrons.push(Chevron {
        x: child.center_x() - CHEVRON_SIZE / 2.0,
        y: child.y - CHEVRON_SIZE,
        direction: Direction::Down,
      });
    }
    if below_rows > 1 {
      graph.join_labels.push((
        child.center_x() - JOIN_LABEL_LEFT,
        child.y - JOIN_LABEL_ABOVE,
        below_rows,
      ));
    }
  }
  graph.right = cards.iter().map(|card| card.right()).fold(ORIGIN_X, f32::max);
  graph.bottom = cards.iter().map(|card| card.bottom()).fold(FIRST_ROW_Y, f32::max);
  if let Some((waiting, dependency)) = closing {
    graph.cycle = cycle_edge(graph, waiting, dependency);
    if graph.cycle.is_some() {
      graph.right += CYCLE_RIGHT;
    }
  }
}

fn connect_in_row(graph: &mut GraphLayout, parent: CardBox, child: CardBox) {
  let y = parent.middle_y();
  graph.segments.push(Segment {
    x1: parent.right(),
    y1: y,
    x2: child.x,
    y2: y,
  });
  graph.chevrons.push(Chevron {
    x: child.x - CHEVRON_SIZE,
    y: y - CHEVRON_SIZE / 2.0,
    direction: Direction::Right,
  });
}

fn connect_rows(graph: &mut GraphLayout, parent: CardBox, child: CardBox) {
  let parallel = graph
    .cards
    .iter()
    .filter(|card| (card.y - child.y).abs() < f32::EPSILON)
    .count()
    > 1;
  let bend = child.y - BEND_ABOVE - if parallel { PARALLEL_BEND_EXTRA } else { 0.0 };
  let (from, to) = (parent.center_x(), child.center_x());
  graph.segments.push(Segment {
    x1: from,
    y1: parent.bottom(),
    x2: from,
    y2: bend,
  });
  graph.segments.push(Segment {
    x1: from.min(to),
    y1: bend,
    x2: from.max(to),
    y2: bend,
  });
  graph.segments.push(Segment {
    x1: to,
    y1: bend,
    x2: to,
    y2: child.y,
  });
}

fn cycle_edge(graph: &GraphLayout, waiting: usize, dependency: usize) -> Option<CycleEdge> {
  let (waiting_card, dependency_card) = (graph.card(waiting)?, graph.card(dependency)?);
  let right = graph.right + CYCLE_RIGHT;
  let top = waiting_card.y - CYCLE_ABOVE;
  let center = waiting_card.center_x();
  let from_y = dependency_card.middle_y();
  let segments = vec![
    Segment {
      x1: dependency_card.right(),
      y1: from_y,
      x2: right,
      y2: from_y,
    },
    Segment {
      x1: right,
      y1: top,
      x2: right,
      y2: from_y,
    },
    Segment {
      x1: center,
      y1: top,
      x2: right,
      y2: top,
    },
    Segment {
      x1: center,
      y1: top,
      x2: center,
      y2: waiting_card.y,
    },
  ];
  Some(CycleEdge {
    waiting,
    dependency,
    segments,
    chevron: Chevron {
      x: center - CHEVRON_SIZE / 2.0,
      y: waiting_card.y - CHEVRON_SIZE,
      direction: Direction::Down,
    },
    label: (center + CYCLE_LABEL_RIGHT, waiting_card.y - CYCLE_LABEL_ABOVE),
  })
}
