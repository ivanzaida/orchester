//! A team task's state on its page (`Task - v2` `jDN0r` rows 14–17: the rail's `State` `VjLQq` in T11a `ujJ6U`): a
//! badge, the state's title in 15/600 over what it means and, for a blocked task, the reason as the person wrote it,
//! then the actions that fit and the one lock line, 10 apart.

use std::sync::Arc;

use lurq::{
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};

use crate::{
  theme::{ExtraSpacing, TeamStateText},
  ui::{
    chip::{BadgeTone, state_badge},
    icon::Icon,
  },
};

pub struct StateCard {
  id: Arc<str>,
  badge: (BadgeTone, Icon, Arc<str>),
  title: Arc<str>,
  reason: Option<Arc<str>>,
  detail: Arc<str>,
  actions: Option<Element>,
  lock: Option<Element>,
}

impl StateCard {
  /// The card `id` (its badge `<id>-badge`, its title `<id>-title`, its detail `<id>-detail`) saying `title` over
  /// `detail`, with `badge` before them.
  pub fn new(
    id: &str,
    badge: (BadgeTone, Icon, Arc<str>),
    title: impl Into<Arc<str>>,
    detail: impl Into<Arc<str>>,
  ) -> Self {
    Self {
      id: id.into(),
      badge,
      title: title.into(),
      reason: None,
      detail: detail.into(),
      actions: None,
      lock: None,
    }
  }

  /// The blocked reason, between the title and the detail (`<id>-reason`).
  pub fn reason(mut self, reason: impl Into<Arc<str>>) -> Self {
    self.reason = Some(reason.into());
    self
  }

  /// The row of actions, under the words.
  pub fn actions(mut self, actions: Option<Element>) -> Self {
    self.actions = actions;
    self
  }

  /// The lock line that says who does the rest, under the actions.
  pub fn lock(mut self, lock: Option<Element>) -> Self {
    self.lock = lock;
    self
  }

  pub fn render(self) -> Element {
    let id = &self.id;
    let (tone, glyph, word) = &self.badge;
    let text = |part: &str, words: &Arc<str>, role| {
      Text::new(words)
        .id(format!("{id}-{part}"))
        .variant(role)
        .width(Dimension::Pct(100.0))
    };
    let badge = Column::new().child(state_badge(&format!("{id}-badge"), *tone, *glyph, word));
    Column::new()
      .id(id.to_string())
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .child(badge)
      .child(text("title", &self.title, TeamStateText::Title.role()))
      .with_children(
        self
          .reason
          .as_ref()
          .map(|reason| text("reason", reason, TeamStateText::Reason.role())),
      )
      .child(text("detail", &self.detail, TeamStateText::Detail.role()))
      .with_children(self.actions)
      .with_children(self.lock)
      .into()
  }
}
