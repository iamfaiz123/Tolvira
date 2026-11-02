//! # Dev Toolbox CSV Engine (`csv-engine`)
//!
//! Thin WASM wrapper around [`tabular_core`], which holds the actual
//! implementation (parsing, cleaning, validation, conversion, merge/split,
//! dedupe, filter/sort, SQL/JSON export, statistics, diff, encodings). See the
//! `tabular-core` crate documentation for the action table.

pub use tabular_core::*;

engine_shared::export_engine!(tabular_core::handle_action, tabular_core::CsvAction);
