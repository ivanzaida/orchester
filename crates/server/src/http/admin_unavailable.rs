//! j8WujU, *The admin app is not running*: the one page the server answers a browser with when its admin app does not
//! answer. The server has no words of its own beyond its console's, which are English. The page is in the design's
//! face, Geist (`assets/fonts`, SIL Open Font License beside it), carried inside the page itself: the admin's own
//! fonts are out of reach while it is down, and the page's CSP lets in nothing from anywhere.

use std::sync::LazyLock;

use axum::{
  http::{HeaderValue, StatusCode, header},
  response::{Html, IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};

const REGULAR: &[u8] = include_bytes!("../../assets/fonts/Geist-Regular.ttf");
const SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/Geist-SemiBold.ttf");

/// Only inline styles and the fonts inside the page.
const POLICY: &str = "default-src 'none'; style-src 'unsafe-inline'; font-src data:; frame-ancestors 'none'";

static PAGE: LazyLock<String> = LazyLock::new(|| {
  include_str!("admin_unavailable.html")
    .replace("{{geist-regular}}", &STANDARD.encode(REGULAR))
    .replace("{{geist-semibold}}", &STANDARD.encode(SEMIBOLD))
});

pub(super) fn not_running() -> Response {
  let mut response = (StatusCode::BAD_GATEWAY, Html(PAGE.as_str())).into_response();
  response
    .headers_mut()
    .insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(POLICY));
  response
}
