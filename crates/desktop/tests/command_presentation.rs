//! Shared command controls retain the source while presenting a compact preview.

use lurq::{
  app::{App, Tree, component::Component, ctx::Ctx},
  clipboard::{copy_to_clipboard, read_from_clipboard},
  core::Signal,
  node::Element,
};
use orchester_desktop::{
  command_row::{CommandPlace, Copied, command_preview, command_row},
  i18n, theme,
};

#[derive(Clone)]
struct Props {
  command: String,
  copied: Signal<Copied>,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.command == other.command
  }
}

impl lurq::app::component::DevtoolsInspectable for Props {}

struct Details;

impl Component for Details {
  type Props = Props;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>();
    command_row(
      ctx,
      "detail",
      "copy",
      &props.command,
      CommandPlace::Transcript,
      &props.copied,
    )
  }
}

#[test]
fn multiline_command_preview_is_single_line_without_changing_one_line_commands() {
  for original in [
    "git status --short",
    "python -c \"print('quoted')\"",
    "  pwsh -File check.ps1  ",
  ] {
    assert_eq!(command_preview(original), original);
  }
  for ending in ["\n", "\r\n", "\r"] {
    let command = format!("pwsh -Command \"{ending}  'quoted'{ending}{ending}  echo done{ending}\"");
    assert_eq!(command_preview(&command), "pwsh -Command \"   'quoted'    echo done \"");
    assert!(command.contains(ending), "the source remains untouched");
  }
}

#[test]
fn multiline_command_copy_feedback_matches_the_original_instead_of_the_preview() {
  let mut app = App::new();
  theme::install(&mut app);
  i18n::install(&app).expect("bundled locales");
  let command = "python -c \"\r\n  print('quoted')\r\n\r\n\"".to_owned();
  let copied = Signal::new(Copied::default());
  let mut tree = Tree::new();
  tree.mount_root::<Details>(
    &mut app,
    Props {
      command: command.clone(),
      copied: copied.clone(),
    },
  );
  for (payload, label) in [(command_preview(&command).into_owned(), "Copy"), (command, "Copied")] {
    copied.set(Copied(Some((payload.into(), true))));
    tree.pass_headless(&mut app);
    let copy = tree.get_element_by_id("copy").expect("copy button");
    assert!(has_text(copy, label));
  }
}

fn has_text(element: lurq::node::element::ElementRef<'_>, text: &str) -> bool {
  element.text_content() == Some(text) || element.children().into_iter().any(|child| has_text(child, text))
}

#[test]
fn multiline_command_details_wrap_long_lines_inside_the_copy_row() {
  let mut app = App::new();
  theme::install(&mut app);
  i18n::install(&app).expect("bundled locales");
  for ending in ["\n", "\r\n"] {
    let command = format!(
      "pwsh -Command \"{ending}  'quoted'{ending}{ending}  # {}{ending}\"",
      "long ".repeat(100)
    );
    for width in [1440, 320] {
      let mut tree = Tree::new();
      tree.resize(width, 960);
      tree.mount_root::<Details>(
        &mut app,
        Props {
          command: command.clone(),
          copied: Signal::new(Copied::default()),
        },
      );
      tree.pass_headless(&mut app);
      let original = tree.get_element_by_id("detail-text").expect("original command");
      assert_eq!(original.text_content(), Some(command.as_str()));
      let text = tree
        .get_element_by_id_mut("detail-text")
        .and_then(|element| element.bounds())
        .expect("text bounds");
      let row = tree
        .get_element_by_id_mut("detail")
        .and_then(|element| element.bounds())
        .expect("row bounds");
      let copy = tree
        .get_element_by_id_mut("copy")
        .and_then(|element| element.bounds())
        .expect("copy bounds");
      assert!(text.height > 26.0);
      assert!(text.y >= row.y && text.y + text.height <= row.y + row.height);
      assert!(text.x + text.width <= copy.x);
      assert!(copy.x + copy.width <= row.x + row.width);
    }
  }
}

#[test]
#[ignore = "requires an available text clipboard; run separately so the user's clipboard can be restored"]
fn multiline_command_copy_action_uses_the_exact_original_payload() {
  let previous = read_from_clipboard().expect("a text clipboard whose previous contents can be restored");
  let mut app = App::new();
  theme::install(&mut app);
  i18n::install(&app).expect("bundled locales");
  for ending in ["\n", "\r\n"] {
    let command = format!(
      "python -c \"{ending}  print('quoted'){ending}{ending}  # {}{ending}\"",
      "long ".repeat(100)
    );
    let copied = Signal::new(Copied::default());
    let mut tree = Tree::new();
    tree.resize(320, 960);
    tree.mount_root::<Details>(
      &mut app,
      Props {
        command: command.clone(),
        copied: copied.clone(),
      },
    );
    tree.pass_headless(&mut app);
    tree.get_element_by_id_mut("copy").expect("copy action").click();
    let payload = copied.get().0.expect("copy records its original payload");
    let actual = read_from_clipboard();
    let restored = copy_to_clipboard(&previous);
    assert!(restored, "restore the previous clipboard text");
    assert_eq!(&*payload.0, command);
    assert!(payload.1, "the clipboard accepts the command");
    assert_eq!(actual.as_deref(), Some(command.as_str()));
  }
}
