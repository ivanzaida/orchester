//! How agent permissions read (`docs/design/orchester-workspace.md`, "Agent permissions"): the
//! options' keys, the allowed commands and the browser's allowed sites as the editor adds and removes
//! them, and the browser turned on and off with the network it needs. Which options are
//! enforceable is decided in core (`orchester_core::permission::capability`); this module only names
//! them. The sentences built from them are each app's.

use orchester_core::{
  connection::{AgentProvider, invocation::Platform},
  permission::{
    AgentPermissions, AllowedCommands, AllowedCommandsError, BrowserPermission, CommandPrefix, CommandPrefixError,
    NetworkAccess,
    browser::{AllowedSite, AllowedSiteError, AllowedSites, AllowedSitesError, BrowserAccess},
    capability::{Mechanism, PermissionOption, PermissionProblem, PermissionSetting, Support, Unavailable, support},
  },
};

use crate::messages::Message;

/// Title of an option (`D2t8u` in `lcwsy`).
pub const fn option_title(option: PermissionOption) -> &'static str {
  match option {
    PermissionOption::ReadOnly => "agents.permissions.option.readOnly.title",
    PermissionOption::EditWorktree => "agents.permissions.option.editWorktree.title",
    PermissionOption::AskBeforeCommands => "agents.permissions.option.askBefore.title",
    PermissionOption::AllowedCommands => "agents.permissions.option.allowed.title",
    PermissionOption::AnyCommand => "agents.permissions.option.any.title",
    PermissionOption::NetworkOff => "agents.permissions.networkOff",
    PermissionOption::NetworkOn => "agents.permissions.networkOn",
  }
}

/// What an option allows on `provider` (`C85wh` in `lcwsy`). Claude Code and native agents also run
/// known-safe read-only commands besides an allowed list, so only their *Allowed commands* says so;
/// only Codex asks about commands that need more than its sandbox, so only its *Any command* says that.
pub fn option_detail(option: PermissionOption, provider: Option<AgentProvider>) -> &'static str {
  match option {
    PermissionOption::AllowedCommands if provider.is_some_and(AgentProvider::is_native) => {
      "agents.permissions.option.allowed.nativeDetail"
    }
    PermissionOption::AllowedCommands if runs_read_only_unlisted(provider) => {
      "agents.permissions.option.allowed.claudeDetail"
    }
    PermissionOption::AnyCommand if provider == Some(AgentProvider::CodexCli) => {
      "agents.permissions.option.any.codexDetail"
    }
    PermissionOption::ReadOnly => "agents.permissions.option.readOnly.detail",
    PermissionOption::EditWorktree => "agents.permissions.option.editWorktree.detail",
    PermissionOption::AskBeforeCommands => "agents.permissions.option.askBefore.detail",
    PermissionOption::AllowedCommands => "agents.permissions.option.allowed.detail",
    PermissionOption::AnyCommand => "agents.permissions.option.any.detail",
    PermissionOption::NetworkOff => "agents.permissions.networkOffDetail",
    PermissionOption::NetworkOn => "agents.permissions.networkOnDetail",
  }
}

pub const fn setting_label(setting: PermissionSetting) -> &'static str {
  match setting {
    PermissionSetting::Repository => "agents.permissions.repository",
    PermissionSetting::Commands => "agents.permissions.commands",
    PermissionSetting::Network => "agents.permissions.network",
  }
}

/// Whether `provider` enforces an allowed list the way that also runs known-safe read-only commands.
pub fn runs_read_only_unlisted(provider: Option<AgentProvider>) -> bool {
  provider.is_some_and(|provider| {
    support(provider, Platform::current(), PermissionOption::AllowedCommands)
      == Support::Enforced(Mechanism::ClaudeAllowedTools)
  })
}

/// Whether `provider` runs agents and enforces every chosen permission here, network off included.
pub fn fully_enforced(permissions: &AgentPermissions, provider: AgentProvider) -> bool {
  provider.runs_agents()
    && permissions
      .enforced_on(provider, Platform::current())
      .is_ok_and(|effective| effective.network_stays_on().is_none())
}

/// Why a connection cannot run `problem`'s permissions, naming the setting it cannot enforce.
pub const fn not_enforceable(problem: &PermissionProblem) -> Message {
  Message::new(match problem {
    PermissionProblem::NotEnforceable { reason, .. } => match reason {
      Unavailable::CannotLimitCommands => "agents.permissions.notEnforceable.allowedCommands",
      Unavailable::CannotConfineCommands => "agents.permissions.notEnforceable.anyCommand",
      Unavailable::CannotBlockNetwork => "agents.permissions.notEnforceable.networkOff",
    },
    PermissionProblem::NoEnforceableOption { setting } => match setting {
      PermissionSetting::Repository => "agents.permissions.notEnforceable.noRepositoryOption",
      PermissionSetting::Commands => "agents.permissions.notEnforceable.noCommandsOption",
      PermissionSetting::Network => "agents.permissions.notEnforceable.noNetworkOption",
    },
  })
}

/// `list` with the command `typed` added, or why it cannot be.
pub fn with_command(list: &AllowedCommands, typed: &str) -> Result<AllowedCommands, Message> {
  let prefix = CommandPrefix::new(typed).map_err(|error| match error {
    CommandPrefixError::Blank => Message::new("agents.permissions.commandInvalid.blank"),
    CommandPrefixError::TooLong => {
      Message::new("agents.permissions.commandInvalid.tooLong").with("max", CommandPrefix::MAX_CHARS.to_string())
    }
    CommandPrefixError::ForbiddenCharacter(character) => {
      Message::new("agents.permissions.commandInvalid.character").with("character", character.to_string())
    }
  })?;
  let mut entries = list.entries().to_vec();
  entries.push(prefix);
  AllowedCommands::new(entries).map_err(|error| match error {
    AllowedCommandsError::Duplicate(command) => {
      Message::new("agents.permissions.commandInvalid.duplicate").with("command", command.to_string())
    }
    AllowedCommandsError::TooMany => {
      Message::new("agents.permissions.commandInvalid.tooMany").with("max", AllowedCommands::MAX_ENTRIES.to_string())
    }
  })
}

/// `list` without the command `removed`.
pub fn without_command(list: &AllowedCommands, removed: &CommandPrefix) -> AllowedCommands {
  let rest = list.entries().iter().filter(|kept| *kept != removed).cloned().collect();
  AllowedCommands::new(rest).expect("a shorter list keeps its entries unique")
}

/// `list` with the browser's allowed site `typed` added, or why it cannot be: an entry that is not a host is refused
/// with its reason.
pub fn with_site(list: &AllowedSites, typed: &str) -> Result<AllowedSites, Message> {
  let invalid = |key: &'static str| Message::new(key).with("site", typed.trim().to_owned());
  let site = AllowedSite::new(typed).map_err(|error| match error {
    AllowedSiteError::Blank => Message::new("agents.permissions.siteInvalid.blank"),
    AllowedSiteError::Spaces => invalid("agents.permissions.siteInvalid.spaces"),
    AllowedSiteError::Scheme => invalid("agents.permissions.siteInvalid.scheme"),
    AllowedSiteError::Path => invalid("agents.permissions.siteInvalid.path"),
    AllowedSiteError::Port => invalid("agents.permissions.siteInvalid.port"),
    AllowedSiteError::Credentials => invalid("agents.permissions.siteInvalid.credentials"),
    AllowedSiteError::Wildcard => invalid("agents.permissions.siteInvalid.wildcard"),
    AllowedSiteError::TooBroad(_) => invalid("agents.permissions.siteInvalid.tooBroad"),
    AllowedSiteError::NotAscii => invalid("agents.permissions.siteInvalid.notAscii"),
    AllowedSiteError::NotAHost(_) => invalid("agents.permissions.siteInvalid.notAHost"),
  })?;
  let mut entries = list.entries().to_vec();
  entries.push(site);
  AllowedSites::new(entries).map_err(|error| match error {
    AllowedSitesError::Duplicate(site) => {
      Message::new("agents.permissions.siteInvalid.duplicate").with("site", site.to_string())
    }
    AllowedSitesError::TooMany => {
      Message::new("agents.permissions.siteInvalid.tooMany").with("max", AllowedSites::MAX_ENTRIES.to_string())
    }
  })
}

/// `list` without the site `removed`.
pub fn without_site(list: &AllowedSites, removed: &AllowedSite) -> AllowedSites {
  let rest = list.entries().iter().filter(|kept| *kept != removed).cloned().collect();
  AllowedSites::new(rest).expect("a shorter list keeps its entries unique")
}

/// Sets `permissions`' network access. The browser needs network on, so network off or to listed hosts only turns the
/// browser off too, keeping its allowed sites.
pub fn set_network(permissions: &mut AgentPermissions, network: NetworkAccess) {
  let browser_kept = matches!(network, NetworkAccess::On);
  permissions.set_network(network);
  if !browser_kept {
    let sites = permissions.browser().sites().clone();
    permissions.set_browser(BrowserPermission::new(BrowserAccess::Off, sites));
  }
}

/// Turns `permissions`' browser on or off, keeping its allowed sites, or says why it cannot be on: it needs network
/// on, not only to listed hosts.
pub fn set_browser_access(permissions: &mut AgentPermissions, access: BrowserAccess) -> Result<(), Message> {
  if access == BrowserAccess::On && !matches!(permissions.network(), NetworkAccess::On) {
    return Err(Message::new("appMcp.errors.browserNeedsNetwork"));
  }
  let sites = permissions.browser().sites().clone();
  permissions.set_browser(BrowserPermission::new(access, sites));
  Ok(())
}

/// Sets `permissions`' allowed sites of the browser, keeping whether it is on.
pub fn set_sites(permissions: &mut AgentPermissions, sites: AllowedSites) {
  let access = permissions.browser().access();
  permissions.set_browser(BrowserPermission::new(access, sites));
}
