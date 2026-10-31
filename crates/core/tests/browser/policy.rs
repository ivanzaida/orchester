//! The policy rows: which steps get the browser, which calls run and which ask, auto-approval and auto mode leaving
//! browser asks to the person, a step's narrowing, and what widens the permission.

use orchester_core::{
  orchester_tools::{
    SERVER, ToolPolicy, auto_approvable,
    browser::{BrowserTool, decide, offered},
  },
  permission::{
    AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess, StepNetwork, StepPermissions,
    browser::{AllowedSite, AllowedSites, BrowserAccess, BrowserPermission, SiteHost},
    widening::Widening,
  },
  run::{
    ApprovalSubject, ToolReach,
    review::{HoldRules, ReviewHold, review_hold},
  },
};

fn sites(entries: &[&str]) -> AllowedSites {
  AllowedSites::new(
    entries
      .iter()
      .map(|entry| AllowedSite::new(entry).expect("a site"))
      .collect(),
  )
  .expect("sites")
}

fn with_browser(repository: RepositoryAccess, network: NetworkAccess, browser: BrowserPermission) -> AgentPermissions {
  let mut permissions = AgentPermissions::new(repository, CommandPermission::AnyInWorktree, network);
  permissions.set_browser(browser);
  permissions
}

fn on(entries: &[&str]) -> BrowserPermission {
  BrowserPermission::new(BrowserAccess::On, sites(entries))
}

fn host(text: &str) -> SiteHost {
  SiteHost::new(text).expect("a host")
}

#[test]
fn browser_is_offered_with_browser_and_network_on_whatever_the_repository_access() {
  for repository in [RepositoryAccess::Write, RepositoryAccess::ReadOnly] {
    let step = with_browser(repository, NetworkAccess::On, on(&[]));
    assert!(offered(&step), "{repository:?}: a read-only step gets the browser too");
    for tool in BrowserTool::ALL {
      assert!(decide(tool, None, &step).is_some(), "{tool:?}");
    }
    let without_network = with_browser(repository, NetworkAccess::Off, on(&["docs.rs"]));
    let off = with_browser(
      repository,
      NetworkAccess::On,
      BrowserPermission::new(BrowserAccess::Off, sites(&["docs.rs"])),
    );
    for step in [without_network, off] {
      assert!(!offered(&step));
      for tool in BrowserTool::ALL {
        assert_eq!(
          decide(tool, Some(&host("docs.rs")), &step),
          None,
          "{tool:?} is not offered"
        );
      }
    }
  }
  let before = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::On,
  );
  assert!(!offered(&before), "off by default");
}

#[test]
fn browser_calls_run_on_allowed_sites_and_ask_elsewhere() {
  let step = with_browser(
    RepositoryAccess::ReadOnly,
    NetworkAccess::On,
    on(&["docs.rs", "*.example.com"]),
  );
  let reads = [
    BrowserTool::Tabs,
    BrowserTool::ReadPage,
    BrowserTool::PageText,
    BrowserTool::Screenshot,
    BrowserTool::Console,
    BrowserTool::Network,
    BrowserTool::Wait,
    BrowserTool::Close,
  ];
  for tool in reads {
    assert_eq!(
      decide(tool, Some(&host("github.com")), &step),
      Some(ToolPolicy::Runs),
      "{tool:?}"
    );
    assert_eq!(decide(tool, None, &step), Some(ToolPolicy::Runs), "{tool:?}");
  }
  let acting = [
    BrowserTool::Open,
    BrowserTool::Navigate,
    BrowserTool::Click,
    BrowserTool::Type,
    BrowserTool::Key,
    BrowserTool::Scroll,
  ];
  for tool in acting {
    for allowed in [
      "docs.rs",
      "staging.example.com",
      "localhost",
      "127.0.0.1",
      "[::1]",
      "app.localhost",
      "kontur.test",
    ] {
      assert_eq!(
        decide(tool, Some(&host(allowed)), &step),
        Some(ToolPolicy::Runs),
        "{tool:?} on {allowed}"
      );
    }
    for other in ["github.com", "example.com", "www.docs.rs"] {
      assert_eq!(
        decide(tool, Some(&host(other)), &step),
        Some(ToolPolicy::Asks),
        "{tool:?} on {other}"
      );
    }
    assert_eq!(
      decide(tool, None, &step),
      Some(ToolPolicy::Asks),
      "{tool:?} on a page without a host"
    );
  }
}

#[test]
fn browser_eval_asks_at_every_call_whatever_the_commands_setting() {
  let step = with_browser(RepositoryAccess::Write, NetworkAccess::On, on(&["docs.rs"]));
  assert_eq!(step.commands(), &CommandPermission::AnyInWorktree);
  for site in [Some(host("docs.rs")), Some(host("localhost")), None] {
    assert_eq!(
      decide(BrowserTool::Eval, site.as_ref(), &step),
      Some(ToolPolicy::Asks),
      "{site:?}"
    );
  }
}

#[test]
fn browser_asks_are_never_answered_by_auto_approval_or_reviewed_by_auto_mode() {
  for tool in BrowserTool::ALL {
    assert!(!auto_approvable(SERVER, tool.as_str()), "{tool:?}");
    assert!(
      auto_approvable("pencil", tool.as_str()),
      "another server's tool of the same name"
    );
    let subject = ApprovalSubject::McpTool {
      server: SERVER.to_owned(),
      tool: tool.as_str().to_owned(),
      reach: ToolReach::Any,
    };
    let step = with_browser(RepositoryAccess::Write, NetworkAccess::On, on(&[]));
    let rules = HoldRules {
      permissions: Some(&step),
      ..HoldRules::default()
    };
    assert_eq!(review_hold(&subject, &rules), Some(ReviewHold::Browser), "{tool:?}");
  }
  assert!(
    auto_approvable(SERVER, "read_external_file"),
    "other tools keep their rule"
  );
}

#[test]
fn browser_step_keeps_its_profiles_browser_and_loses_it_with_its_network() {
  let profile = with_browser(RepositoryAccess::Write, NetworkAccess::On, on(&["docs.rs"]));
  let as_profile = profile.narrowed(RepositoryAccess::ReadOnly, &StepPermissions::default());
  assert_eq!(as_profile.browser(), profile.browser());
  assert!(offered(&as_profile), "a read-only step of the profile gets the browser");
  let no_network = StepPermissions {
    network: StepNetwork::Off,
    ..StepPermissions::default()
  };
  let narrowed = profile.narrowed(RepositoryAccess::Write, &no_network);
  assert_eq!(narrowed.browser(), profile.browser(), "kept, for the step's record");
  assert!(!offered(&narrowed), "a step without network gets no browser");
}

#[test]
fn browser_widens_when_turned_on_or_given_a_site_not_covered() {
  let permissions = |browser| with_browser(RepositoryAccess::Write, NetworkAccess::On, browser);
  let off = permissions(BrowserPermission::off());
  let docs = permissions(on(&["docs.rs"]));
  assert_eq!(docs.widenings_from(&off), [Widening::Browser, Widening::BrowserSite]);
  assert_eq!(permissions(on(&[])).widenings_from(&off), [Widening::Browser]);
  let kept_off = permissions(BrowserPermission::new(BrowserAccess::Off, sites(&["docs.rs"])));
  assert_eq!(
    docs.widenings_from(&kept_off),
    [Widening::Browser],
    "turning it on again with its own sites"
  );
  assert!(kept_off.widenings_from(&docs).is_empty(), "turning it off narrows");
  assert!(
    permissions(BrowserPermission::new(
      BrowserAccess::Off,
      sites(&["docs.rs", "github.com"])
    ))
    .widenings_from(&off)
    .is_empty(),
    "sites kept while off allow nothing"
  );
  let more = permissions(on(&["docs.rs", "github.com"]));
  assert_eq!(more.widenings_from(&docs), [Widening::BrowserSite]);
  assert!(docs.widenings_from(&more).is_empty(), "removing a site narrows");
  let domain = permissions(on(&["*.example.com"]));
  assert!(
    permissions(on(&["staging.example.com"]))
      .widenings_from(&domain)
      .is_empty(),
    "a site the domain covers"
  );
  assert_eq!(
    domain.widenings_from(&permissions(on(&["staging.example.com"]))),
    [Widening::BrowserSite]
  );
}
