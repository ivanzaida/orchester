//! The code viewer in the real headless window, on Kontur made a real git repository: the tree and a file at a
//! version, selected lines, the versions, refused paths, binary files, *Go to file*, search within the file,
//! *Open file* on the Changes tab, and that a file's text never reaches the MCP tree.
mod harness;
mod mcp_client;
mod page;
mod privacy;
#[path = "../support/mod.rs"]
mod support;
