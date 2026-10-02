//! Shared command controls retain the source while presenting a compact preview.

use lurq::{
  app::{App, Tree, component::Component, ctx::Ctx, events::ScrollPhase},
  clipboard::{copy_to_clipboard, read_from_clipboard},
  components::Column,
  core::{ElementRect, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_desktop::{
  command_row::{CommandPlace, Copied, command_preview, command_row},
  i18n,
  theme::{self, ExtraTypography},
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
    // Inside a column, as in the transcript, so the window does not stretch the box.
    Column::new().width(Dimension::Pct(100.0)).child(command_row(
      ctx,
      "detail",
      "copy",
      &props.command,
      CommandPlace::Transcript,
      &props.copied,
    ))
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
    assert_eq!(command_preview(&command), "pwsh -Command \" 'quoted' echo done \"");
    assert!(command.contains(ending), "the source remains untouched");
  }
  let indented = "pwsh -NoProfile -Command \"
  $files = Get-ChildItem crates/desktop/src
  foreach ($f in $files) {  Write-Output $f  }
\"";
  assert_eq!(
    command_preview(indented),
    "pwsh -NoProfile -Command \" $files = Get-ChildItem crates/desktop/src foreach ($f in $files) {  Write-Output $f  } \"",
    "indentation reads as one space; spaces within a line stay"
  );
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
fn multiline_command_details_centre_a_one_line_command_on_copy() {
  let mut app = App::new();
  theme::install(&mut app);
  i18n::install(&app).expect("bundled locales");
  let mut tree = Tree::new();
  tree.resize(960, 960);
  tree.mount_root::<Details>(
    &mut app,
    Props {
      command: "pnpm test src/auth".into(),
      copied: Signal::new(Copied::default()),
    },
  );
  tree.pass_headless(&mut app);
  let (text, row, copy) = (
    bounds(&mut tree, "detail-text"),
    bounds(&mut tree, "detail"),
    bounds(&mut tree, "copy"),
  );
  assert_eq!(row.height, 38.0, "the box of `BQgqg`");
  assert!((text.y + text.height / 2.0 - (copy.y + copy.height / 2.0)).abs() < 0.5);
}

/// Where `id` is laid out.
fn bounds(tree: &mut Tree, id: &str) -> ElementRect {
  tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

#[test]
fn multiline_command_details_keep_each_line_and_scroll_it_sideways_beside_copy() {
  let mut app = App::new();
  theme::install(&mut app);
  i18n::install(&app).expect("bundled locales");
  let line = {
    let style = app.theme().typography_style(ExtraTypography::Output.role());
    style.font_size * style.line_height
  };
  for ending in [
    "
", "
",
  ] {
    let command = format!(
      "pwsh -Command \"{ending}  'quoted'{ending}{ending}  # {}{ending}\"",
      "long ".repeat(100)
    );
    for width in [1440, 320] {
      let copied = Signal::new(Copied::default());
      let mut tree = Tree::new();
      tree.resize(width, 960);
      tree.mount_root::<Details>(
        &mut app,
        Props {
          command: command.clone(),
          copied: copied.clone(),
        },
      );
      tree.pass_headless(&mut app);
      let original = tree.get_element_by_id("detail-text").expect("original command");
      assert_eq!(original.text_content(), Some(command.as_str()));
      let (text, scroll) = (bounds(&mut tree, "detail-text"), bounds(&mut tree, "detail-scroll"));
      let (row, copy) = (bounds(&mut tree, "detail"), bounds(&mut tree, "copy"));
      assert!(
        (text.height - 5.0 * line).abs() < 0.5,
        "five recorded lines stay five lines: nothing wraps"
      );
      assert!(text.width > scroll.width, "the long line runs past the box");
      assert!(scroll.x + scroll.width <= copy.x, "Copy is outside the scrolling text");
      assert_eq!(
        (copy.y - row.y, row.x + row.width - copy.x - copy.width),
        (6.0, 6.0),
        "Copy at the top right"
      );
      assert!(
        (text.y + line / 2.0 - (copy.y + copy.height / 2.0)).abs() < 0.5,
        "the first line is centred on Copy"
      );
      assert_eq!(
        (text.x - row.x, text.y - row.y),
        (12.0, 10.0),
        "the output block's inset"
      );

      let (x, y) = (scroll.x + scroll.width / 2.0, scroll.y + scroll.height / 2.0);
      tree.scroll(x, y, 0.0, 0.0, ScrollPhase::Start);
      tree.scroll(x, y, -200.0, 0.0, ScrollPhase::Scroll);
      tree.scroll(x, y, 0.0, 0.0, ScrollPhase::End);
      tree.pass_headless(&mut app);
      let scrolled = bounds(&mut tree, "detail-text");
      assert_eq!(scrolled.x, text.x - 200.0, "the lines scroll sideways");
      assert_eq!(bounds(&mut tree, "copy"), copy, "Copy does not move");
      assert_eq!(bounds(&mut tree, "detail"), row, "the box does not move");
      copied.set(Copied(Some((command.as_str().into(), true))));
      tree.pass_headless(&mut app);
      assert!(has_text(tree.get_element_by_id("copy").expect("copy"), "Copied"));
      assert_eq!(
        bounds(&mut tree, "detail-text").x,
        scrolled.x,
        "copying keeps the scroll"
      );
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
