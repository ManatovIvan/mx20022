//! Generated and hand-written types for ISO 20022 financial messages.
//!
//! # Feature flags
//!
//! Each message family is behind a feature flag:
//! - `head` — Business Application Header (`head.001`)
//! - `pacs` — Payments Clearing and Settlement (`pacs.002`, `pacs.004`,
//!   `pacs.008`, `pacs.009`, `pacs.028`) at their current versions
//! - `legacy-pacs` — older pacs versions still in active use
//!   (`pacs.002.001.10/12`, `pacs.008.001.08/10`); implies `pacs`
//! - `pain` — Payment Initiation
//! - `camt` — Cash Management
//! - `all` — enables all families (including `legacy-pacs`)

pub mod common;
pub mod generated;
