# May 2026 maintenance pass

This document summarises the May 2026 maintenance batch derived from
the ranked review report. Each item from the input list has one
status row below; PRs are linked where they exist.

## Item status

| # | Item | Status | PR |
|---|------|--------|----|
| 1 | Replace `"UNKNOWNXXXXX"` placeholder BIC in pacs.008 → MT103 and pacs.009 → MT202 with `TranslationError::MissingField` | completed | [#18](https://github.com/socrates8300/mx20022/pull/18) (merged) |
| 2 | Add `TranslationOptions { lossy: bool }` gate for silently dropped MT103 fields (`:53/:54/:71F/:71G/:72`) | deferred | — |
| 3 | Derive `camt.053` entry currency from MT940 `:60F:` opening balance | completed | [#19](https://github.com/socrates8300/mx20022/pull/19) (merged) |
| 4 | Export the four dormant generated pacs versions (`pacs.002.001.10`, `pacs.002.001.12`, `pacs.008.001.08`, `pacs.008.001.10`) | completed | [#20](https://github.com/socrates8300/mx20022/pull/20) (merged) |
| 5 | Curated `prelude` and quickstart docs on the umbrella `mx20022` crate | completed | [#21](https://github.com/socrates8300/mx20022/pull/21) (merged) |
| 6 | SWIFT line-wrap budget for MX → MT party and remittance fields | completed | [#22](https://github.com/socrates8300/mx20022/pull/22) |
| 7 | Wrap `quick_xml::DeError` with detected `MessageId` context | completed | [#23](https://github.com/socrates8300/mx20022/pull/23) (merged) |
| 8 | Expand `mx20022-translate` crate docs with quickstart and supported-pair matrix | completed | [#24](https://github.com/socrates8300/mx20022/pull/24) |

## Deferral details

### Item 2 — `TranslationOptions { lossy }` for MT → pacs.008 silent drops

**Status:** deferred — no code written, no branch open.

**What was found.** `mt103_to_pacs008` silently drops `:53:` (sender's
correspondent), `:54:` (receiver's correspondent), `:71F:` and `:71G:`
(senders' / receivers' charges), and `:72:` (sender-to-receiver info)
when translating to pacs.008 — these MT fields have no clean
pacs.008.001.13 equivalent. The current behavior is to record a
warning per field and return `Ok` with the lossy result. The same
pattern exists in `mt202_to_pacs009.rs:92-100`.

**Why deferred.** The original triage proposed
`TranslationOptions { lossy: bool }` with `default = false` (strict).
Implementing that defaults-to-strict shape changes the *behavior* of
existing callers — every previously-Ok translation that contained any
of these fields now returns `Err`. That is not an API surface change
(no signature changed) but it is a semantic change of the same
magnitude as the missing-BIC fix in PR #18. The maintenance prompt's
"defer, don't stub" guidance applies because:

1. The default direction (strict vs lossy) is a design call the
   maintainer should make. An additive variant
   (`mt103_to_pacs008_with_options`, default-behavior preserved) would
   ship a strict mode for new callers without breaking existing ones —
   but choosing that shape is "narrowing the item to its easy slice"
   per the prompt's scope-collapse warning.
2. The bug class is broader than just `mt103_to_pacs008`: the same
   silent-drop pattern lives in `mt202_to_pacs009`. A
   well-scoped fix should either gate both translators or explain why
   only one is in scope.

**What is needed to unblock.** Maintainer call on:
- Should the default be strict (errors on data-loss) or lossy (warnings, current behavior)?
- Should `mt202_to_pacs009` get the same gate in the same PR?
- Is `TranslationOptions` the right shape, or should each translator just gain a `_strict` variant?

The wrap-truncation work in PR #22 introduces `&mut TranslationWarnings`
into the call site of `build_party_field`. That same pattern can be
reused for the lossy gate.

## Test coverage decisions

Every completed PR added at least one test that demonstrates the
specific bug from the review item is fixed. The decisions below are
listed for the record:

| PR | Coverage |
|----|----------|
| [#18](https://github.com/socrates8300/mx20022/pull/18) | Added 4 new tests (`test_pacs008_to_mt103_missing_{sender,receiver}_bic_errors`, `test_pacs009_to_mt202_missing_{sender,receiver}_bic_errors`) plus a `pacs008_to_mt103_empty_cdt_trf_tx` regression test. Test fixtures in three modules and `testdata/mt/{mt103,mt202}.txt` updated with `:52A:` / `:57A:` so existing roundtrip tests still propagate BICs. |
| [#19](https://github.com/socrates8300/mx20022/pull/19) | Added 2 new tests in `mt940_to_camt053`: `test_mt940_to_camt053_entry_currency_inherited_from_opening_balance` (success path) and `test_mt940_to_camt053_empty_opening_currency_errors` (degenerate input → tagged warnings). |
| [#20](https://github.com/socrates8300/mx20022/pull/20) | No new test added because the verification is "modules are reachable under `legacy-pacs`"; that is enforced by `cargo check -p mx20022-model --features legacy-pacs` (compile failure if the `pub mod` declarations are wrong) and exercised by `cargo test --workspace --all-features` (since `--all-features` includes `legacy-pacs`). A unit test that imports `Document` from each new module would not catch anything `cargo check` does not. |
| [#21](https://github.com/socrates8300/mx20022/pull/21) | The 4 new doctests embedded in the prelude documentation are the test coverage. They both verify the API works and serve as the user-facing examples. |
| [#22](https://github.com/socrates8300/mx20022/pull/22) | Added 8 unit tests for `wrap_lines` (boundaries, word-wrap, hard-cut, overflow-Err, panic-on-zero) plus 3 integration tests against `pacs008_to_mt103` (long-but-wrappable name, oversize name overflow, oversize remittance overflow). |
| [#23](https://github.com/socrates8300/mx20022/pull/23) | Added 3 new tests in `mx20022-parse::de::tests`: detected-envelope failure path, undetected-envelope fallback path, and matching-envelope success path. |
| [#24](https://github.com/socrates8300/mx20022/pull/24) | The 2 new doctests embedded in the crate-level docs are the test coverage; they double as the rendered examples. |

## Out-of-scope observations

See [`FOLLOWUPS.md`](../../FOLLOWUPS.md) at the repo root for
observations captured during this batch but explicitly out of scope.
That file currently lists:

- pre-existing `clippy --all-targets` failures in test code
  (Rust 1.95 `single_char_pattern` lint; CI does not run with
  `--all-targets`)
- `camt.053 → MT940` still emits the `"UNKNOWNXXXXX"` placeholder for
  sender BIC — different root cause from the pacs cases (camt.053
  structurally has no BIC), requires an API change
- `extract_bic_from_fi` falls back from `bicfi` to `nm` — a misleading
  helper name, deliberately left in place because the test fixtures
  rely on it
