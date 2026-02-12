// PHASE 3: TUI module - refactored from monolithic tui.rs

pub mod types;
pub mod handlers;

pub use types::*;
pub use handlers::*;

// Re-export main entry point
pub use crate::commands::tui::{TuiArgs, handle};
