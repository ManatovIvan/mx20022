# Out-of-scope observations from May 2026 maintenance pass

Items captured here were noticed while landing the ranked items in
`docs/maintenance/2026-05-summary.md` but are out of scope for the PR they
were observed in. Each notes where it was found, what it is, and a
suggested fix shape for whoever picks it up.

## Pre-existing `clippy --all-targets` failures in test code

- **Where:** `crates/mx20022-model/src/common/builder.rs:61-63` (and related
  test files surfaced by `cargo clippy --workspace --all-targets --all-features`).
- **What:** `assert!(s.contains("a"), ...)` triggers
  `clippy::single_char_pattern` under Rust 1.95's clippy. CI does not run
  with `--all-targets`, so these are silent in pipeline today; they only
  fail under the maintainer-specified verification command.
- **Status on `main` before this batch:** broken (reproducible with
  `git checkout main && cargo clippy --workspace --all-targets --all-features -- -D warnings`).
- **Fix shape:** swap the single-char string patterns for `char` literals.
  Trivial; would also be a good occasion to add `--all-targets` to the CI
  clippy job so this doesn't drift again.

## camt.053 → MT940 still emits `"UNKNOWNXXXXX"` placeholder for sender BIC

- **Where:** `crates/mx20022-translate/src/mappings/camt053_to_mt940.rs:78`.
- **What:** Looks like the same bug as the pacs.008/pacs.009 placeholders
  fixed in PR #1, but the root cause is different: camt.053 statements
  structurally do not carry a sender/receiver BIC, so there is nothing to
  read from the source document. The MT940 application header still needs
  one. The placeholder hides this gap.
- **Why deferred:** fixing this requires either an API change
  (`camt053_to_mt940(doc, sender_bic: &BIC)`) or a builder-style
  `TranslationContext { sender_bic, ... }`. PR #1 was sized to remove only
  the placeholders that *did* have a fixable source; this one is a
  signature change and therefore a v2 conversation per the maintenance
  prompt's no-API-breakage rule.
- **Fix shape:** add an explicit `sender_bic: Option<&str>` parameter (or
  a context struct), error when absent, and update callers to thread it
  through.

## `extract_bic_from_fi` falls back from `bicfi` to `nm`

- **Where:** `crates/mx20022-translate/src/mappings/pacs008_to_mt103.rs`
  (`extract_bic_from_fi`) and `pacs009_to_mt202.rs` (`extract_bic_from_fi6`).
- **What:** A function named `extract_bic_from_fi` that returns the FI's
  *name* when the BIC is missing is misleading. PR #1 deliberately did
  not change this behavior because the test fixtures rely on the
  `:52A:NAME` → `nm` → `extract_bic_from_fi` fallback path to roundtrip.
  The cleaner long-term fix is to teach `party_to_fi_id` /
  `party_info_to_fi6` to detect BIC-shaped party values (8 or 11 chars,
  `[A-Z0-9]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?`) and route them to
  `bicfi` instead of `nm`. Then `extract_bic_from_fi` can drop the `nm`
  fallback.
- **Why deferred:** scope creep relative to PR #1's surface (one bug, two
  call sites). Touching the `party_to_fi_id` helper would change behavior
  for every translation path that goes through it.
- **Fix shape:** add `is_bic_shaped(s: &str) -> bool` to
  `mt::fields::common`, use it in `party_to_fi_id` /
  `party_info_to_fi6` to choose `bicfi` vs `nm`, then strip the `nm`
  fallback from `extract_bic_from_fi*`.
