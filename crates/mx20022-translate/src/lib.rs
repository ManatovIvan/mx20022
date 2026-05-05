//! Bidirectional MT ↔ MX translation for ISO 20022 messages.
//!
//! This crate translates between SWIFT MT FIN messages and their
//! corresponding ISO 20022 MX (XML) representations. It provides:
//!
//! - A SWIFT MT message parser ([`mt`]) that decomposes a raw MT
//!   message into its block structure (blocks 1, 2, 3, 4, 5) and
//!   then into typed field structs ([`mt::fields::mt103::Mt103`],
//!   [`mt::fields::mt202::Mt202`], [`mt::fields::mt940::Mt940`]).
//! - Translation helpers in [`mappings`] that convert each parsed
//!   MT struct into the corresponding pacs / camt `Document` from
//!   `mx20022-model`, and back.
//! - A [`mappings::charset`] module covering SWIFT FIN character-set
//!   conversion and SWIFT-MT line-wrapping.
//! - Translation result and error types
//!   ([`mappings::TranslationResult`], [`mappings::TranslationError`])
//!   that distinguish hard failures from data-loss warnings
//!   ([`mappings::TranslationWarning`]).
//!
//! # Supported message-type pairs
//!
//! | Direction | Source           | Target              |
//! |-----------|------------------|---------------------|
//! | MT → MX   | MT103            | pacs.008.001.13     |
//! | MX → MT   | pacs.008.001.13  | MT103               |
//! | MT → MX   | MT202            | pacs.009.001.10     |
//! | MX → MT   | pacs.009.001.10  | MT202               |
//! | MT → MX   | MT940            | camt.053.001.11     |
//! | MX → MT   | camt.053.001.11  | MT940               |
//!
//! Other MT message types (MT202COV, MT900, MT910, MT942) and other
//! pacs / camt versions are not yet wired up.
//!
//! # Quick start: MT103 → pacs.008.001.13
//!
//! ```
//! use mx20022_translate::mappings::mt103_to_pacs008::mt103_to_pacs008;
//! use mx20022_translate::mt::{self, fields::mt103::parse_mt103};
//!
//! let raw = "\
//! {1:F01BANKBEBBAXXX0000000000}\
//! {2:I103BANKDEFFXXXXN}\
//! {3:{108:MYREF}}\
//! {4:\n\
//! :20:REF123\n\
//! :23B:CRED\n\
//! :32A:230615EUR1000,00\n\
//! :50K:ACME CORP\n\
//! :59:JANE SMITH\n\
//! :71A:SHA\n\
//! -}\
//! {5:{CHK:ABCDEF1234}}";
//!
//! // Step 1: parse the raw MT into block structure.
//! let msg = mt::parse(raw).unwrap();
//! assert_eq!(msg.message_type(), Some("103"));
//!
//! // Step 2: parse Block 4 into typed MT103 fields.
//! let mt103 = parse_mt103(&msg.block4).unwrap();
//! assert_eq!(mt103.senders_reference, "REF123");
//!
//! // Step 3: translate to pacs.008.001.13.
//! let result = mt103_to_pacs008(&mt103, "MSG-1", "2023-06-15T10:00:00").unwrap();
//! let doc = &result.message;
//! let tx = &doc.fi_to_fi_cstmr_cdt_trf.cdt_trf_tx_inf[0];
//! assert_eq!(tx.intr_bk_sttlm_amt.value.0, "1000.00");
//! assert_eq!(tx.intr_bk_sttlm_amt.ccy.0, "EUR");
//!
//! // result.warnings carries any data-loss notes, e.g. when an MT
//! // field has no clean pacs.008 equivalent.
//! for w in &result.warnings.warnings {
//!     eprintln!("{}: {}", w.field, w.message);
//! }
//! ```
//!
//! # Quick start: pacs.008.001.13 → MT103
//!
//! ```
//! # use mx20022_translate::mappings::{mt103_to_pacs008::mt103_to_pacs008, pacs008_to_mt103::pacs008_to_mt103};
//! # use mx20022_translate::mt::{self, fields::mt103::parse_mt103};
//! # let raw = "{1:F01BANKBEBBAXXX0000000000}{2:I103BANKDEFFXXXXN}{3:{108:R}}{4:\n:20:R\n:23B:CRED\n:32A:230615EUR1,00\n:50K:A\n:59:B\n:71A:SHA\n-}{5:{CHK:0}}";
//! # let mt103 = parse_mt103(&mt::parse(raw).unwrap().block4).unwrap();
//! # let doc = mt103_to_pacs008(&mt103, "M", "2023-06-15T10:00:00").unwrap().message;
//! let result = pacs008_to_mt103(&doc).unwrap();
//! assert!(result.message.contains(":20:"));
//! assert!(result.message.contains(":32A:"));
//! ```
//!
//! See the `roundtrip` and `translate_mt103` examples in the umbrella
//! `mx20022` crate for end-to-end demos that include XML
//! serialization.

pub mod mappings;
pub mod mt;
