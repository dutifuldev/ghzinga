pub mod layout;
pub mod markdown;
pub mod resource;
pub mod scrollbar;
pub mod spacing;
pub mod symbols;
pub mod theme;
pub mod time;
pub mod width;

pub use layout::ViewRects;
pub use resource::render_app;
pub use scrollbar::ScrollbarMode;
pub use spacing::SpacingMode;
pub use symbols::{SymbolMode, Symbols};
pub use theme::{Palette, ThemeName};
pub use width::{
    normalize_fixed_width, ContentWidthMode, DEFAULT_FIXED_CONTENT_WIDTH, FIXED_CONTENT_WIDTH_STEP,
    MAX_FIXED_CONTENT_WIDTH, MIN_FIXED_CONTENT_WIDTH,
};
