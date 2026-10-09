//! `message_text` words the application layer's messages with `orchester-workspace`'s own locale files,
//! loaded under their namespace, filling in their values.

use std::path::PathBuf;

use lurq::{
  app::{
    App, Tree,
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::{Column, Text},
  node::Element,
};
use orchester_app::{i18n, ui::message::message_text};
use orchester_store::StoreError;
use orchester_workspace::{
  messages::{self, Message},
  runner::state::RunnerFailure,
};

#[derive(Clone, PartialEq)]
struct Shown(Vec<Message>);

impl DevtoolsInspectable for Shown {}

/// Each message of its props as a line of text.
struct Lines;

impl Component for Lines {
  type Props = Shown;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Shown(shown) = ctx.props::<Shown>().clone();
    shown.iter().fold(Column::new().id("lines"), |lines, message| {
      lines.child(Text::new(&message_text(ctx, message)))
    })
  }
}

fn words(shown: Vec<Message>) -> Vec<String> {
  let mut app = App::new();
  i18n::install(&app).expect("bundled locales load");
  let mut tree = Tree::new();
  tree.mount_root::<Lines>(&mut app, Shown(shown));
  tree.pass_headless(&mut app);
  let lines = tree.get_element_by_id("lines").expect("the lines show");
  lines
    .children()
    .into_iter()
    .filter_map(|line| line.text_content().map(str::to_owned))
    .collect()
}

#[test]
fn message_text_words_workspace_messages_with_their_values() {
  let missing = RunnerFailure::MissingExecutable {
    path: PathBuf::from("bin/orchester-runner"),
  };
  let newer = StoreError::SchemaTooNew { found: 9, supported: 7 };
  assert_eq!(
    words(vec![
      messages::runner_failure(&missing),
      messages::store_error(&newer),
      messages::runner_not_restarted(5, Some("14:02")),
    ]),
    [
      "The runner program was not found at bin/orchester-runner.",
      "The data was written by a newer version of Orchester.",
      "Tried 5 times since 14:02. Interrupted runs keep their last checkpoint. Nothing runs until the runner starts.",
    ]
  );
}
