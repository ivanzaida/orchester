//! What the step strip draws, read from the flow of the edit the panel holds: a chip per step in the order they run,
//! the steps that run side by side in one group, a hand where the run waits for the person, and the Deliver step last.
//! Steps one line cannot show (the flow has no layout for them) keep the order they are stored in, one chip each.

use std::sync::Arc;

use orchester_core::resource::{PipelineEdit, StepKey};
use orchester_workspace::pipelines::flow::FlowRead;

/// One chip: a step, or the Deliver step (`key` none), numbered as the flow numbers them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Chip {
  pub key: Option<StepKey>,
  pub number: usize,
  pub name: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Piece {
  Chip(Chip),
  /// Steps that run side by side, in one frame.
  Parallel(Vec<Chip>),
  /// The run waits for the person's approval here.
  Gate,
}

/// The pieces of the strip, `deliver` being the name of the Deliver step.
pub(super) fn of(read: &FlowRead, edit: &PipelineEdit, deliver: &str) -> Vec<Piece> {
  let mut pieces = Vec::new();
  match &read.flow {
    Ok(flow) => {
      for stage in &flow.stages {
        let chips: Vec<Chip> = stage
          .cards
          .iter()
          .map(|card| Chip {
            key: Some(card.key.clone()),
            number: card.number,
            name: card.name.clone(),
          })
          .collect();
        match <[Chip; 1]>::try_from(chips) {
          Ok([only]) => pieces.push(Piece::Chip(only)),
          Err(chips) => pieces.push(Piece::Parallel(chips)),
        }
        pieces.extend(stage.gates.iter().map(|_| Piece::Gate));
      }
      if let Some(card) = &flow.deliver {
        pieces.push(Piece::Chip(Chip {
          key: None,
          number: card.number,
          name: deliver.into(),
        }));
      }
    }
    Err(_) => pieces.extend(edit.steps().iter().enumerate().map(|(index, step)| {
      Piece::Chip(Chip {
        key: Some(step.key.clone()),
        number: index + 1,
        name: step.name.as_str().into(),
      })
    })),
  }
  pieces
}

fn chips(pieces: &[Piece]) -> impl Iterator<Item = &Chip> {
  pieces.iter().flat_map(|piece| match piece {
    Piece::Chip(chip) => std::slice::from_ref(chip),
    Piece::Parallel(chips) => chips.as_slice(),
    Piece::Gate => &[],
  })
}

/// The steps in the order they run, which the keys ← and → walk through.
pub(super) fn order(pieces: &[Piece]) -> Vec<StepKey> {
  chips(pieces).filter_map(|chip| chip.key.clone()).collect()
}

/// Where step `key` stands: its number, and how many steps the pipeline has (the Deliver step is not counted).
pub(super) fn place(pieces: &[Piece], key: &StepKey) -> Option<(usize, usize)> {
  let number = chips(pieces).find(|chip| chip.key.as_ref() == Some(key))?.number;
  Some((number, order(pieces).len()))
}

/// The steps that run before step `key`: the places where a request for changes can send the run back.
pub(super) fn upstream(pieces: &[Piece], key: &StepKey) -> Vec<Chip> {
  let mut before = Vec::new();
  for piece in pieces {
    let holds = match piece {
      Piece::Chip(chip) => chip.key.as_ref() == Some(key),
      Piece::Parallel(chips) => chips.iter().any(|chip| chip.key.as_ref() == Some(key)),
      Piece::Gate => false,
    };
    if holds {
      break;
    }
    match piece {
      Piece::Chip(chip) => before.push(chip.clone()),
      Piece::Parallel(chips) => before.extend(chips.iter().cloned()),
      Piece::Gate => {}
    }
  }
  before
}
