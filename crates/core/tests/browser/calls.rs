//! A browser call's input read into checked values: the URLs `browser_open` takes and refuses with their reason, and
//! the tabs, refs, key presses and limits of the other tools.

use orchester_core::orchester_tools::browser::{
  BlockedUrl, BrowserCall, BrowserCallError, BrowserTool, ClickTarget, ElementRef, HistoryStep, KeyPress, Modifier,
  MouseButton, PageUrl, ScrollDirection, ScrollTarget, TabId, WaitFor,
};
use serde_json::{Value, json};

fn call(tool: &str, arguments: &Value) -> BrowserCall {
  BrowserCall::parse(tool, arguments).unwrap_or_else(|error| panic!("{tool} {arguments}: {error}"))
}

fn invalid(tool: &str, arguments: &Value) -> String {
  match BrowserCall::parse(tool, arguments) {
    Err(BrowserCallError::Invalid { message, .. }) => message,
    other => panic!("{tool} {arguments} is refused as invalid, not {other:?}"),
  }
}

#[test]
fn browser_open_takes_http_and_https_urls_by_their_host() {
  for (url, host) in [
    ("https://docs.rs/serde/latest/serde/", "docs.rs"),
    ("http://localhost:5173/orders?id=7#top", "localhost"),
    ("HTTPS://Staging.Example.COM", "staging.example.com"),
    ("http://127.0.0.1:8080", "127.0.0.1"),
    ("http://[::1]:3000/", "[::1]"),
    ("https://docs.rs:443", "docs.rs"),
  ] {
    let parsed = PageUrl::parse(url).unwrap_or_else(|error| panic!("{url}: {error}"));
    assert_eq!(parsed.host().as_str(), host, "{url}");
    assert_eq!(parsed.as_str(), url);
  }
  let open = call("browser_open", &json!({"url": "https://docs.rs"}));
  assert_eq!(open.opened_url().map(|url| url.host().as_str()), Some("docs.rs"));
  assert_eq!(
    open,
    BrowserCall::Open {
      url: PageUrl::parse("https://docs.rs").expect("a URL"),
      new_tab: false
    }
  );
}

#[test]
fn browser_open_blocks_other_urls_with_the_reason() {
  for (url, scheme) in [
    ("file:///C:/Users/me/secrets.txt", "file"),
    ("chrome://settings", "chrome"),
    ("edge://settings/passwords", "edge"),
    ("javascript:alert(document.cookie)", "javascript"),
    ("JavaScript:void 0", "javascript"),
    ("data:text/html,<script>x</script>", "data"),
    ("about:blank", "about"),
    ("view-source:https://docs.rs", "view-source"),
    ("ftp://files.example.com", "ftp"),
  ] {
    assert_eq!(PageUrl::parse(url), Err(BlockedUrl::Scheme(scheme.to_owned())), "{url}");
    assert_eq!(
      BrowserCall::parse("browser_open", &json!({"url": url})),
      Err(BrowserCallError::Blocked(BlockedUrl::Scheme(scheme.to_owned()))),
      "{url}"
    );
  }
  for url in ["docs.rs/serde", "localhost:5173", ""] {
    assert_eq!(PageUrl::parse(url), Err(BlockedUrl::NoScheme(url.to_owned())), "{url}");
  }
  assert_eq!(
    PageUrl::parse("https://me:hunter2@staging.example.com/"),
    Err(BlockedUrl::Credentials),
    "no browser tool takes a secret"
  );
  for url in [
    "https://",
    "https:docs.rs",
    "https://docs.rs:99999",
    "https://exa mple.com",
    "https://a_b.dev",
  ] {
    assert_eq!(PageUrl::parse(url), Err(BlockedUrl::NotAUrl(url.to_owned())), "{url}");
  }
  assert_eq!(
    PageUrl::parse(&format!("https://docs.rs/{}", "a".repeat(9000))),
    Err(BlockedUrl::TooLong)
  );
}

#[test]
fn browser_calls_read_tabs_refs_and_choices() {
  assert_eq!(
    call("browser_tabs", &Value::Null),
    BrowserCall::Tabs,
    "no arguments is none"
  );
  assert_eq!(
    call("browser_navigate", &json!({"to": "reload"})),
    BrowserCall::Navigate {
      tab: None,
      to: HistoryStep::Reload
    },
    "no tab is the current one"
  );
  let tab = TabId::new("t-2_b").expect("a tab id");
  assert_eq!(
    call("browser_click", &json!({"tab": "t-2_b", "ref": "e12"})),
    BrowserCall::Click {
      tab: Some(tab),
      target: ClickTarget::Element(ElementRef::new("e12").expect("a ref")),
      button: MouseButton::Left,
      count: 1,
    }
  );
  assert_eq!(
    call(
      "browser_click",
      &json!({"x": 10, "y": 20, "button": "middle", "count": 3})
    ),
    BrowserCall::Click {
      tab: None,
      target: ClickTarget::Point { x: 10, y: 20 },
      button: MouseButton::Middle,
      count: 3
    }
  );
  assert_eq!(
    call("browser_scroll", &json!({"direction": "up"})),
    BrowserCall::Scroll {
      tab: None,
      target: ScrollTarget::Page {
        direction: ScrollDirection::Up,
        pixels: None
      }
    }
  );
  assert_eq!(
    call("browser_wait", &json!({"ref": "e4"})),
    BrowserCall::Wait {
      tab: None,
      until: WaitFor::Element(ElementRef::new("e4").expect("a ref")),
      timeout_seconds: 10
    }
  );
  assert_eq!(
    call("browser_read_page", &json!({})),
    BrowserCall::ReadPage {
      tab: None,
      query: None,
      interactive_only: false,
      max_chars: 20_000
    }
  );
  assert_eq!(
    call("browser_eval", &json!({"expression": "document.title"})),
    BrowserCall::Eval {
      tab: None,
      expression: "document.title".to_owned()
    }
  );
}

#[test]
fn browser_calls_refuse_input_the_tools_cannot_take() {
  for (tool, arguments) in [
    ("browser_click", json!({})),
    ("browser_click", json!({"ref": "e1", "x": 1, "y": 2})),
    ("browser_click", json!({"x": 1})),
    ("browser_click", json!({"ref": "e1", "count": 4})),
    ("browser_click", json!({"ref": "12"})),
    ("browser_click", json!({"tab": "../t1", "ref": "e1"})),
    ("browser_type", json!({"ref": "e1"})),
    ("browser_type", json!({"ref": "e1", "text": "x".repeat(10_001)})),
    ("browser_key", json!({"keys": []})),
    ("browser_key", json!({"keys": ["Hyper+A"]})),
    ("browser_key", json!({"keys": ["Ctrl+Ctrl+A"]})),
    ("browser_key", json!({"keys": ["Enter now"]})),
    ("browser_scroll", json!({})),
    ("browser_scroll", json!({"ref": "e1", "direction": "down"})),
    ("browser_scroll", json!({"direction": "down", "pixels": 0})),
    ("browser_screenshot", json!({"ref": "e1", "full_page": true})),
    ("browser_wait", json!({})),
    ("browser_wait", json!({"text": "Saved", "ref": "e1"})),
    ("browser_wait", json!({"text": "Saved", "timeout": 61})),
    ("browser_read_page", json!({"max_chars": 0})),
    ("browser_read_page", json!({"query": "two\nlines"})),
    ("browser_console", json!({"limit": 501})),
    ("browser_navigate", json!({"to": "home"})),
    ("browser_open", json!({})),
    ("browser_open", json!({"url": "https://docs.rs", "tab": "t1"})),
    ("browser_eval", json!({})),
    ("browser_tabs", json!([])),
  ] {
    let message = invalid(tool, &arguments);
    assert!(!message.is_empty(), "{tool} {arguments}");
  }
  assert_eq!(
    BrowserCall::parse("browser_cookies", &json!({})),
    Err(BrowserCallError::UnknownTool("browser_cookies".to_owned()))
  );
  assert!(
    BrowserCallError::Invalid {
      tool: BrowserTool::Click,
      message: "m".to_owned()
    }
    .to_string()
    .starts_with("invalid input for `browser_click`")
  );
}

#[test]
fn browser_key_presses_name_modifiers_and_one_key() {
  let press = KeyPress::new("Ctrl+Shift+ArrowDown").expect("a key press");
  assert_eq!(press.modifiers(), [Modifier::Ctrl, Modifier::Shift]);
  assert_eq!(press.key(), "ArrowDown");
  assert_eq!(
    KeyPress::new("cmd+a").expect("a key press").modifiers(),
    [Modifier::Meta]
  );
  assert_eq!(KeyPress::new("Ctrl++").expect("a key press").key(), "+");
  assert_eq!(KeyPress::new("+").expect("a key press").key(), "+");
  assert_eq!(KeyPress::new("é").expect("a key press").key(), "é");
  for text in ["", "Ctrl+", "Ctrl+Space bar", "Tab\t", "AVeryLongKeyNameIndeed"] {
    assert!(KeyPress::new(text).is_err(), "{text:?}");
  }
}
