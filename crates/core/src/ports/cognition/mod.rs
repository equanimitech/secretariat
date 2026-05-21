//! Cognition ports — four sibling traits, one bounded responsibility each.
//!
//! - [`CognitionRouting`] — one-shot classification (route_capture).
//! - [`CognitionAg`] — one-shot distillation of a body into AG fields
//!   (title / lede / summary) at envelope-write time.
//! - [`CognitionLaunching`] — plan a CLI invocation for external-terminal
//!   launch (`sec launch`, Tauri `launch_channel_from_pane`).
//! - [`CognitionSession`] — drive an in-process streaming multi-turn
//!   conversation (tab-strip primary UI, future digest composer, future
//!   review co-pilot).
//!
//! Adapters in `infrastructure/cognition/` implement zero, one, or
//! several of these — substrate capabilities, not all the same shape.

pub mod ag;
pub mod launching;
pub mod routing;
pub mod session;

pub use ag::{AgFields, CognitionAg};
pub use launching::{CognitionLaunching, LaunchPlan, LauncherError};
pub use routing::{CognitionError, CognitionRouting, RouteSuggestion};
pub use session::{CognitionSession, SessionError, SessionEvent, SessionRef};
