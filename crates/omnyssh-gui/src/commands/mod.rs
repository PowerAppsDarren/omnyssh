//! IPC command handlers, one module per domain (tech-gui.md §3.1). Commands are
//! thin: validate input, call the core, return a DTO or an error.

pub mod hosts;
pub mod snippets;
