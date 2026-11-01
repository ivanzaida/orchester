//! What the window's own MCP shows of a page: its chrome, and nothing the page holds, in the tree or in a screenshot.
//! The client is in process, over the production tools and input gate, and a stand-in render engine paints the whole
//! window one color, so a pixel of another color is what the capture painted over it.
//!
//! `unsafe` here only because `Tree::pass` renders for a surface with a window handle, and a test has no window: the
//! handle below stands for none, and the engine it reaches never reads it.

#![allow(unsafe_code)]

use std::{num::NonZeroIsize, sync::Arc, thread, time::Duration};

use base64::{Engine, engine::general_purpose::STANDARD};
use image::RgbaImage;
use lurq::{
  app::render_engine::{CapturedFrame, RenderCaptureTarget, RenderEngine, RenderFrameCapture},
  layout::render_list::RenderList,
  mcp::rmcp::{RoleClient, ServiceExt, model::CallToolRequestParams, service::RunningService},
};
use orchester_app::{
  appearance::{Appearance, ThemeSetting},
  mcp::{self, McpSettings},
};
use orchester_ipc::browser::{
  BrowserDialog, BrowserEvent, BrowserMode, BrowserTabId, DialogKind, ElementBox, SelectId, SelectList, SelectOption,
};
use parking_lot::Mutex;
use raw_window_handle::{
  DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, Win32WindowHandle, WindowHandle,
};
use serde_json::{Value, json};

use super::{
  input::frame,
  support::{Browser, browser_installed, state},
};

/// What the stand-in engine paints the window with.
const PAINT: [u8; 4] = [10, 200, 90, 255];
/// What lurq paints over sensitive text in a capture made for inspection.
const COVERED: [u8; 4] = [128, 128, 128, 255];
/// How far past a text's clip lurq covers it, in physical pixels: the clip's slop and the rounding outwards.
const COVER_SLOP: u32 = 2;

const MESSAGE: &str = "Delete orders-sync?";
const OPTIONS: [&str; 2] = ["Fixed delay", "Exponential with jitter"];

/// A surface a pass renders for. Its handles are never used: the engine below ignores them.
struct Surface;

impl HasWindowHandle for Surface {
  fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
    let handle = Win32WindowHandle::new(NonZeroIsize::MIN);
    // SAFETY: no one dereferences the handle: `Painter` is the only engine of a tree on this surface, and ignores it.
    Ok(unsafe { WindowHandle::borrow_raw(handle.into()) })
  }
}

impl HasDisplayHandle for Surface {
  fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
    Ok(DisplayHandle::windows())
  }
}

/// Paints the whole window [`PAINT`] and hands a capture over through the pipeline's target, as a GPU engine does.
struct Painter {
  /// The opacity of each glyph the last frame scales up: the cover's.
  scaled: Arc<Mutex<Vec<f32>>>,
}

impl RenderEngine for Painter {
  fn resize(&mut self, _width: u32, _height: u32) {}

  fn render(&mut self, list: &RenderList, _: WindowHandle<'_>, _: DisplayHandle<'_>) -> bool {
    *self.scaled.lock() = list
      .glyphs
      .iter()
      .filter(|glyph| glyph.transform[0] > 1.0)
      .map(|glyph| glyph.color[3])
      .collect();
    true
  }

  fn supports_frame_capture(&self) -> bool {
    true
  }

  fn render_with_capture(
    &mut self,
    list: &RenderList,
    window: WindowHandle<'_>,
    display: DisplayHandle<'_>,
    capture: Option<RenderFrameCapture>,
  ) -> bool {
    self.render(list, window, display);
    if let Some(capture) = capture {
      let RenderCaptureTarget::Bytes(deliver) = capture.target else {
        panic!("an inspection capture is delivered as pixels");
      };
      deliver(Ok(CapturedFrame {
        width: capture.width,
        height: capture.height,
        rgba: PAINT.repeat((capture.width * capture.height) as usize),
      }));
    }
    true
  }
}

/// An MCP client of the Browser window's tree, which it renders with [`Painter`].
pub(super) struct Tools {
  tools: RunningService<RoleClient, ()>,
  server: tokio::task::JoinHandle<()>,
  scaled: Arc<Mutex<Vec<f32>>>,
}

impl Drop for Tools {
  fn drop(&mut self) {
    self.server.abort();
  }
}

impl Tools {
  pub(super) fn new(browser: &mut Browser) -> Self {
    let scaled = Arc::new(Mutex::new(Vec::new()));
    let shared = scaled.clone();
    browser
      .tree
      .set_render_engine_factory(move || Box::new(Painter { scaled: shared.clone() }));
    let window = &browser.window;
    let appearance = Appearance::new(ThemeSetting::Dark, Arc::default(), None);
    let config = mcp::config(
      McpSettings::with_window_control(true),
      &window.link,
      &appearance,
      window.launch.services(),
      window.app.i18n(),
    );
    let tools = browser.tree.enable_mcp(config.hosted());
    let gate = window.link.input_gate();
    gate.attach(tools.clone());
    let (client, server) = tokio::io::duplex(64 * 1024);
    let server = window.data.runtime.spawn(async move {
      gate
        .guard(tools.service())
        .serve(server)
        .await
        .expect("server initializes")
        .waiting()
        .await
        .expect("server ends");
    });
    let tools = window
      .data
      .runtime
      .block_on(().serve(client))
      .expect("client initializes");
    Self { tools, server, scaled }
  }

  /// What the tool `name` answers.
  pub(super) fn call(&self, browser: &mut Browser, name: &str, arguments: &Value) -> Value {
    let peer = self.tools.peer().clone();
    let params =
      CallToolRequestParams::new(name.to_owned()).with_arguments(arguments.as_object().expect("arguments").clone());
    let request = browser
      .window
      .data
      .runtime
      .spawn(async move { peer.call_tool(params).await.expect("tool answers") });
    while !request.is_finished() {
      browser.tree.drain_mcp_requests(&mut browser.window.app);
      browser.tree.tick_futures();
      browser.tree.request_redraw();
      browser.tree.pass(&mut browser.window.app, &Surface);
      thread::sleep(Duration::from_millis(5));
    }
    let result = browser.window.data.runtime.block_on(request).expect("call finished");
    serde_json::to_value(result).expect("tool result")
  }

  /// `lurq_screenshot` of the whole window.
  fn screenshot(&self, browser: &mut Browser) -> RgbaImage {
    let answer = self.call(browser, "lurq_screenshot", &json!({}));
    let data = answer["content"][0]["data"]
      .as_str()
      .unwrap_or_else(|| panic!("a screenshot, not {answer}"));
    let png = STANDARD.decode(data).expect("base64");
    image::load_from_memory(&png).expect("a PNG").to_rgba8()
  }
}

/// The window on the person's tab with a frame, its JavaScript dialog waiting and a `<select>` list open.
fn page_with_dialog_and_select() -> Option<Browser> {
  if !browser_installed() {
    return None;
  }
  let mut browser = Browser::open(false);
  browser.feed(state("P1", BrowserMode::Headless).0);
  browser.until_shown("browser-page");
  let page = browser.bounds("browser-page");
  frame(&browser, "P1", 1, (page.width as u32, page.height as u32));
  browser.until_shown("browser-page-frame");
  browser.feed(BrowserEvent::DialogOpened {
    tab: BrowserTabId::new("P1"),
    dialog: BrowserDialog {
      kind: DialogKind::Confirm,
      message: MESSAGE.to_owned(),
      default_prompt: String::new(),
    },
  });
  browser.feed(BrowserEvent::SelectOpened {
    tab: BrowserTabId::new("P1"),
    list: SelectList {
      select: SelectId::new(7),
      options: OPTIONS
        .into_iter()
        .map(|label| SelectOption {
          label: label.to_owned(),
          selected: false,
          disabled: false,
        })
        .collect(),
      element: ElementBox {
        x: 55,
        y: 303,
        width: 552,
        height: 34,
      },
    },
  });
  browser.until_shown("browser-dialog-message");
  browser.until_shown("browser-select-option-1");
  Some(browser)
}

fn assert_nothing_of_the_page(answer: &Value, tool: &str) {
  let answer = answer.to_string();
  for text in [MESSAGE, OPTIONS[0], OPTIONS[1]] {
    assert!(!answer.contains(text), "{tool} shows the page's {text:?}: {answer}");
  }
}

#[test]
fn browser_mcp_tree_and_inspection_carry_none_of_the_pages_text() {
  let Some(mut browser) = page_with_dialog_and_select() else {
    return;
  };
  assert_eq!(browser.text("browser-dialog-message"), MESSAGE, "the person reads it");
  assert_eq!(
    browser.text("browser-select-option-1"),
    OPTIONS[1],
    "the person reads it"
  );
  let tools = Tools::new(&mut browser);

  let tree = tools.call(&mut browser, "lurq_read_tree", &json!({}));
  assert_nothing_of_the_page(&tree, "lurq_read_tree");
  let tree = tree.to_string();
  assert!(tree.contains("sensitive"), "what the page holds is marked: {tree}");
  for chrome in [
    "browser-address",
    "Your tab",
    "Pull requests",
    "browser-dialog-message",
    "browser-select-option-1",
    "browser-page-cover",
    "browser-dialog-ok",
  ] {
    assert!(tree.contains(chrome), "the window's {chrome} stays readable: {tree}");
  }

  for id in [
    "browser-dialog-message",
    "browser-select-option-0",
    "browser-select-option-1",
    "browser-page",
  ] {
    let inspected = tools.call(&mut browser, "lurq_inspect", &json!({"query": id}));
    assert_nothing_of_the_page(&inspected, "lurq_inspect");
    assert!(inspected.to_string().contains(id), "{id} is found: {inspected}");
  }
  let buttons = tools.call(&mut browser, "lurq_inspect", &json!({"role": "button"}));
  assert_nothing_of_the_page(&buttons, "lurq_inspect of the buttons");
  let found = tools.call(&mut browser, "lurq_find", &json!({"query": MESSAGE}));
  assert!(
    !found.to_string().contains("browser-dialog"),
    "a search of the tree finds no text of the page: {found}"
  );
}

#[test]
fn browser_inspection_screenshot_paints_over_the_page_and_keeps_the_chrome() {
  let Some(mut browser) = page_with_dialog_and_select() else {
    return;
  };
  let page = browser.bounds("browser-page");
  let tools = Tools::new(&mut browser);
  let capture = tools.screenshot(&mut browser);
  assert_eq!(capture.dimensions(), (1280, 800));

  let (left, top) = (page.x as u32, page.y as u32);
  let (right, bottom) = ((page.x + page.width) as u32, (page.y + page.height) as u32);
  assert!(top > COVER_SLOP, "the toolbar lies over the page");
  for (x, y, pixel) in capture.enumerate_pixels() {
    let inside = (left..right).contains(&x) && (top..bottom).contains(&y);
    let beyond = x + COVER_SLOP < left || x >= right + COVER_SLOP || y + COVER_SLOP < top || y >= bottom + COVER_SLOP;
    if inside {
      assert_eq!(pixel.0, COVERED, "the page at {x},{y} is painted over");
    } else if beyond {
      assert_eq!(pixel.0, PAINT, "the window's chrome at {x},{y} stays");
    }
  }

  let scaled = tools.scaled.lock().clone();
  assert!(!scaled.is_empty(), "the cover is drawn");
  assert!(
    scaled.iter().all(|opacity| *opacity == 0.0),
    "the person sees nothing of it: {scaled:?}"
  );
}
