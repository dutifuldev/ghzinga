mod placeholder;
pub mod state;
pub mod update;

pub(crate) use placeholder::loading_resource_placeholder;
pub use state::{AppState, BlockId, Tab};
pub use update::{apply_event, AppEvent, AppIntent};
