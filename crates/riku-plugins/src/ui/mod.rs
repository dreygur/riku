//! UI panel seam: plugin-contributed dashboard panels (`PLUGIN_PROTOCOL.md` §7.5).

mod dispatch;

pub use dispatch::{run_ui_panel, PanelField, PanelSection, UiPanelResponse};
