//! The app's window without a screen: the real root with the theme and the bundled locales, laid out
//! headless at the design's 1440 × 900.

use lurq::{
  app::{App, Tree},
  node::element::ElementRef,
};
use orchester_app::{
  i18n,
  mcp::{McpStatus, WindowLink},
  navigation::Navigation,
  root::{Root, RootProps},
  theme::{self, Mode},
};

pub struct Window {
  pub app: App,
  pub tree: Tree,
  pub link: WindowLink,
}

impl Window {
  pub fn open() -> Self {
    let mut app = App::new();
    theme::install(&mut app, Mode::Dark);
    i18n::install(&app).expect("bundled locales load");
    let mut tree = Tree::new();
    tree.resize(1440, 900);
    let link = WindowLink::default();
    tree.mount_root::<Root>(
      &mut app,
      RootProps {
        link: link.clone(),
        mcp: McpStatus::Unavailable,
      },
    );
    tree.pass_headless(&mut app);
    Self { app, tree, link }
  }

  pub fn navigation(&self) -> Navigation {
    self.link.navigation().expect("the root attached its navigation")
  }

  /// Lays the window out again after a change.
  pub fn pass(&mut self) {
    self.tree.pass_headless(&mut self.app);
  }

  /// The texts inside the element `id`, icon glyphs left out; `None` when no element has the id.
  pub fn texts(&mut self, id: &str) -> Option<Vec<String>> {
    let element = self.tree.get_element_by_id(id)?;
    let mut all = Vec::new();
    collect(element, &mut all);
    all.retain(|text| !text.chars().all(|glyph| ('\u{e000}'..='\u{f8ff}').contains(&glyph)));
    Some(all)
  }
}

fn collect(element: ElementRef<'_>, into: &mut Vec<String>) {
  into.extend(element.text_content().map(str::to_owned));
  for child in element.children() {
    collect(child, into);
  }
}
