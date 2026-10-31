//! IPC command handlers, grouped by feature (one file per UI feature area).
//!
//! Rules for every command:
//! - returns `Result<T, AppError>` — the UI translates `AppError.code`;
//! - contains no business logic, only calls into library crates;
//! - is registered in `lib.rs` and listed in PROJECT_MAP.md → "IPC map";
//! - has a typed wrapper in `apps/desktop/src/shared/ipc/commands.ts`.

pub mod app;
pub mod flow;
pub mod project;
pub mod site_map;
