//! Runtime independent from the Iced presentation layer.

mod commands;
mod events;
mod service_handle;
mod supervisor;

pub use commands::ClientCommand;
pub use events::{ClientEvent, RuntimeSnapshot};
pub use service_handle::RuntimeHandle;
pub use supervisor::spawn;
