//! The page (`Page` `Dqgop`): the latest screencast frame of the tab shown, drawn as one image as it was decoded off
//! the UI thread (`orchester_workspace::browser::RgbaFrame`), and the person's pointer, wheel, keys and an input
//! method's text sent back to the tab, in the page's CSS pixels (`input`). Over it Orchester draws what a headless
//! frame does not show (`docs/design/browser.md`, "What frames do not show"): the ring and name where an agent acted
//! (`mark`, `02` `UKAkY`), a JavaScript dialog of the person's tab over the page only (`dialog`, `03` `t08Ur`), and a
//! `<select>` list under its field in the menu's look (`select`, `04` `r31HX`). What a page holds stays out of
//! inspection: the frame is covered in the captures made for it (`cover`), and the text drawn from the page is
//! sensitive.

pub mod activity;
mod cover;
mod dialog;
mod input;
mod mark;
mod select;

use std::sync::Arc;

use lurq::{
  app::{component::Component, ctx::Ctx, theme::PaletteColor},
  components::{Image, Stack},
  core::{ElementRef, Signal},
  images::ImageData,
  node::{Element, dimension::Dimension},
};
use orchester_ipc::browser::{BrowserTabId, TabOwner};
use orchester_workspace::browser::RgbaFrame;
use parking_lot::Mutex;

use super::state::Showing;

/// The page's size in logical pixels and the frame's in CSS pixels, to place what is drawn over it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scale {
  /// Logical pixels per CSS pixel, across and down.
  pub x: f32,
  pub y: f32,
}

impl Scale {
  /// A CSS pixel is a logical pixel: before the first frame, the page is the area's own size.
  const SAME: Self = Self { x: 1.0, y: 1.0 };

  /// `frame`'s CSS pixels drawn over `width` × `height` logical pixels.
  fn of(frame: &RgbaFrame, (width, height): (f32, f32)) -> Self {
    let (css_width, css_height) = frame.css_size();
    Self {
      x: width / css_width.max(1) as f32,
      y: height / css_height.max(1) as f32,
    }
  }
}

pub struct Page {
  element: ElementRef,
  size: Signal<Option<(f32, f32)>>,
  /// The image of the frame drawn last, by its sequence: one image per frame.
  image: Mutex<Option<(BrowserTabId, u64, ImageData)>>,
  input: input::PageInput,
}

impl Component for Page {
  type Props = Showing;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      element: ctx.element_ref(),
      size: ctx.signal(None),
      image: Mutex::new(None),
      input: input::PageInput::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let showing = ctx.props::<Showing>().clone();
    let size = self.measured();
    let view = showing.view.as_ref();
    let tab = showing.shown().cloned();
    let frame = view.and_then(|view| view.frame()).cloned();
    let image = frame.as_ref().map(|frame| self.image_of(frame));
    let scale = frame
      .as_ref()
      .zip(size)
      .map_or(Scale::SAME, |(frame, size)| Scale::of(frame, size));
    let mut page = Stack::new()
      .id("browser-page")
      .describe("tab", tab.as_ref().map_or("", BrowserTabId::as_str).to_owned())
      .describe(
        "frame",
        frame
          .as_ref()
          .map_or(String::new(), |frame| frame.sequence().to_string()),
      )
      .ref_element(self.element.clone())
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .clip()
      .background(PaletteColor::SurfacePanel);
    if let Some(image) = image {
      page = page
        .child(
          Image::new(image)
            .id("browser-page-frame")
            .stretch()
            .width(Dimension::Pct(100.0))
            .height(Dimension::Pct(100.0)),
        )
        .child(cover::cover());
    }
    let Some(tab) = tab else {
      return page;
    };
    let person = view
      .and_then(|view| view.tabs().iter().find(|open| open.id == tab))
      .is_some_and(|open| open.owner == TabOwner::Person);
    let page = self.input.attach(
      ctx,
      page,
      (&showing, &tab),
      (self.element.clone(), frame.as_ref().map(|frame| frame.css_size())),
    );
    let mark = mark::mark(ctx, &showing, &tab, scale);
    let select = select::list(ctx, &showing, &tab, scale);
    let dialog = person.then(|| dialog::dialog(ctx, &showing, &tab)).flatten();
    page.with_children(mark).with_children(select).with_children(dialog)
  }
}

impl Page {
  /// The page's size, once it was laid out; watched from now on.
  fn measured(&self) -> Option<(f32, f32)> {
    let size = self.size.clone();
    self.element.observe_rect(move |rect| {
      let measured = (rect.width, rect.height);
      if rect.width > 0.0 && size.get_untracked() != Some(measured) {
        size.set(Some(measured));
      }
    });
    self.size.get()
  }

  /// `frame` as an image, made once per frame.
  fn image_of(&self, frame: &Arc<RgbaFrame>) -> ImageData {
    let mut image = self.image.lock();
    if let Some((tab, sequence, data)) = image.as_ref()
      && tab == frame.tab()
      && *sequence == frame.sequence()
    {
      return data.clone();
    }
    let data = ImageData::from_rgba(frame.pixels().to_vec(), frame.width(), frame.height());
    *image = Some((frame.tab().clone(), frame.sequence(), data.clone()));
    data
  }
}
