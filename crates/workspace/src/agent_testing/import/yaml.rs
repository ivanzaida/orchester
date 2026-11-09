//! A YAML file of the checkout as a tree. Aliases are copies of what they name, so a small file could stand for a
//! huge tree (a "billion laughs"): its size with every alias expanded is counted from the parser's events first, and
//! a file past [`MAX_NODES`] is refused before any of it is copied.

use std::collections::HashMap;

use yaml_rust2::{
  Event, Yaml, YamlLoader,
  parser::{MarkedEventReceiver, Parser},
  scanner::Marker,
};

use crate::messages::Message;

/// The most nodes a file may expand to; a compose file has a few hundred.
const MAX_NODES: usize = 100_000;

/// The first document of `text`; `None` for a file without one.
pub(super) fn load(text: &str) -> Result<Option<Yaml>, Message> {
  let syntax =
    |error: yaml_rust2::ScanError| Message::new("services.import.unreadable.yaml").with("reason", error.to_string());
  let mut size = Size::default();
  Parser::new_from_str(text).load(&mut size, true).map_err(syntax)?;
  if size.nodes > MAX_NODES {
    return Err(Message::new("services.import.unreadable.yamlTooLarge").with("max", MAX_NODES.to_string()));
  }
  Ok(YamlLoader::load_from_str(text).map_err(syntax)?.into_iter().next())
}

/// The nodes a document expands to, an alias counting as many as the node its anchor marks.
#[derive(Default)]
struct Size {
  nodes: usize,
  /// The expanded size of each anchored node.
  anchored: HashMap<usize, usize>,
  /// Each open collection's anchor and the count once it opened.
  open: Vec<(usize, usize)>,
}

impl Size {
  fn add(&mut self, nodes: usize) {
    self.nodes = self.nodes.saturating_add(nodes);
  }
}

impl MarkedEventReceiver for Size {
  fn on_event(&mut self, event: Event, _: Marker) {
    match event {
      Event::Scalar(_, _, anchor, _) => {
        self.add(1);
        if anchor > 0 {
          self.anchored.insert(anchor, 1);
        }
      }
      Event::SequenceStart(anchor, _) | Event::MappingStart(anchor, _) => {
        self.add(1);
        self.open.push((anchor, self.nodes));
      }
      Event::SequenceEnd | Event::MappingEnd => {
        if let Some((anchor, opened)) = self.open.pop()
          && anchor > 0
        {
          self.anchored.insert(anchor, self.nodes - opened + 1);
        }
      }
      Event::Alias(anchor) => {
        let nodes = self.anchored.get(&anchor).copied().unwrap_or(1);
        self.add(nodes);
      }
      Event::Nothing | Event::StreamStart | Event::StreamEnd | Event::DocumentStart | Event::DocumentEnd => {}
    }
  }
}

/// The text of a scalar: a string, number or boolean as written. `None` for a collection, a null or an unknown alias.
pub(super) fn text(node: &Yaml) -> Option<String> {
  match node {
    Yaml::String(text) | Yaml::Real(text) => Some(text.clone()),
    Yaml::Integer(number) => Some(number.to_string()),
    Yaml::Boolean(value) => Some(value.to_string()),
    Yaml::Array(_) | Yaml::Hash(_) | Yaml::Alias(_) | Yaml::Null | Yaml::BadValue => None,
  }
}

/// A mapping's entries whose keys are text, in order.
pub(super) fn entries(node: &Yaml) -> Vec<(String, &Yaml)> {
  node
    .as_hash()
    .map(|hash| {
      hash
        .iter()
        .filter_map(|(key, value)| Some((text(key)?, value)))
        .collect()
    })
    .unwrap_or_default()
}
