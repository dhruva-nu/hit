//! Shared chrome: header with breadcrumb, key-chip footer, modal overlays,
//! JSON syntax coloring, loading lines.

mod chrome;
mod docs;
mod json;
mod modal;
mod state;

pub use chrome::{content_panel, draw_footer, draw_header, rule};
pub use docs::endpoint_docs_lines;
pub use json::{colorize_json_line, loading_line};
pub use modal::{centered_fixed, draw_modal};
pub use state::empty_state;
