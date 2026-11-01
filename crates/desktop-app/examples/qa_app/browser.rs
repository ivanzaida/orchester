//! The QA copy's Browser window (ORC-379): when the seed left a browser fixture beside the data
//! (`orchester_app::test_support::browser_qa`), the window's state is played from it, its page a real local page
//! (`browser_page.html`) that this computer's Chrome, Edge, Brave or Chromium renders headless to a PNG, sent as a
//! screencast frame. Without such a browser the tabs play without a page, and the log says why.

use std::{
  fs,
  io::Cursor,
  path::{Path, PathBuf},
  process::Command,
};

use image::ImageFormat;
use orchester_app::{
  services::Services,
  test_support::browser_qa::{Fixture, Page, play},
};
use tokio::task::JoinHandle;

const PAGE: &str = include_str!("browser_page.html");
/// The device pixels per CSS pixel the page is rendered at.
const SCALE: u32 = 2;

/// Plays the fixture beside `database`, if the seed left one.
pub fn start(services: &Services, database: &Path) -> Result<Option<JoinHandle<()>>, String> {
  let Some(fixture) = Fixture::beside(database)? else {
    return Ok(None);
  };
  let folder = database
    .parent()
    .map(Path::to_path_buf)
    .ok_or("the database has no folder")?;
  tracing::info!(state = fixture.state.name(), "the QA Browser window plays its fixture");
  Ok(Some(play(services, fixture, move |query, size| {
    render(&folder, query, size)
      .inspect_err(|error| tracing::warn!(%error, "the QA browser's page was not rendered"))
      .ok()
  })))
}

/// The sample page with its fields as `query` says, `width` × `height` CSS pixels, as a JPEG.
fn render(folder: &Path, query: &str, (width, height): (u32, u32)) -> Result<Page, String> {
  let program = orchester_exec::browser::find().map_err(|error| error.to_string())?;
  let html = folder.join("browser-qa-page.html");
  fs::write(&html, PAGE).map_err(|error| format!("{}: {error}", html.display()))?;
  let named: String = query
    .chars()
    .map(|glyph| if glyph.is_ascii_alphanumeric() { glyph } else { '-' })
    .collect();
  let shot = folder.join(format!("browser-qa-{named}-{height}.png"));
  let profile: PathBuf = folder.join("browser-qa-profile");
  let output = Command::new(program.path())
    .arg("--headless=new")
    .arg("--disable-gpu")
    .arg("--hide-scrollbars")
    .arg("--no-first-run")
    .arg("--no-default-browser-check")
    .arg(format!("--force-device-scale-factor={SCALE}"))
    .arg(format!("--user-data-dir={}", profile.display()))
    .arg(format!("--window-size={width},{height}"))
    .arg(format!("--screenshot={}", shot.display()))
    .arg(format!("file://{}?{query}", html.display()))
    .output()
    .map_err(|error| format!("{}: {error}", program.path().display()))?;
  if !shot.is_file() {
    return Err(format!(
      "{} made no screenshot: {}",
      program.name(),
      String::from_utf8_lossy(&output.stderr)
    ));
  }
  let image = image::open(&shot).map_err(|error| format!("{}: {error}", shot.display()))?;
  let mut jpeg = Cursor::new(Vec::new());
  image
    .to_rgb8()
    .write_to(&mut jpeg, ImageFormat::Jpeg)
    .map_err(|error| error.to_string())?;
  Ok(Page {
    jpeg: jpeg.into_inner(),
    css: (width, height),
  })
}
