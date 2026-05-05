//! Translation from pacs.008.001.13 to MT103 Customer Credit Transfer text.

use mx20022_model::generated::pacs::pacs_008_001_13 as pacs008;

use crate::mappings::{
    charset::wrap_lines,
    error::{TranslationError, TranslationResult, TranslationWarnings},
    helpers::{code_to_charges, format_mt_message, iso_date_to_yymmdd},
};

/// SWIFT MT line length budget for narrative fields like :50K:, :59:, :70:.
const MT_LINE_LEN: usize = 35;
/// SWIFT MT max line count for :50K: / :59: party blocks and :70: remittance.
const MT_PARTY_MAX_LINES: usize = 4;

/// Translate a `pacs.008.001.13` `Document` to an MT103 message string.
///
/// Only the first `CdtTrfTxInf` entry is translated.  A warning is added when
/// the document contains more than one transaction.
///
/// # Errors
///
/// Returns [`TranslationError::MissingField`] when a required pacs.008 field
/// is absent.
pub fn pacs008_to_mt103(
    doc: &pacs008::Document,
) -> Result<TranslationResult<String>, TranslationError> {
    let mut warnings = TranslationWarnings::default();

    let fi_to_fi = &doc.fi_to_fi_cstmr_cdt_trf;

    if fi_to_fi.cdt_trf_tx_inf.len() > 1 {
        warnings.add(
            "CdtTrfTxInf",
            "document contains multiple transactions; only the first is translated",
        );
    }

    let tx = fi_to_fi
        .cdt_trf_tx_inf
        .first()
        .ok_or_else(|| TranslationError::MissingField {
            field: "CdtTrfTxInf".into(),
            context: "pacs008_to_mt103".into(),
        })?;

    // :20: — sender's reference from EndToEndId
    let reference = &tx.pmt_id.end_to_end_id.0;

    // :23B:
    let bank_op_code = "CRED";

    // :32A: value date + currency + amount
    let value_date_iso = tx
        .intr_bk_sttlm_dt
        .as_ref()
        .map_or("000101", |d| d.0.as_str());
    let value_date_swift = iso_date_to_yymmdd(value_date_iso)?;
    let ccy = &tx.intr_bk_sttlm_amt.ccy.0;
    let amt_dot = &tx.intr_bk_sttlm_amt.value.0;
    let amt_swift = amt_dot.replace('.', ",");
    let sttlm_field = format!("{value_date_swift}{ccy}{amt_swift}");

    // :50K: ordering customer (Dbtr + DbtrAcct)
    let ordering_party = build_party_field(&tx.dbtr, tx.dbtr_acct.as_ref(), "50K", &mut warnings);

    // :59: beneficiary (Cdtr + CdtrAcct)
    let beneficiary = build_party_field(&tx.cdtr, tx.cdtr_acct.as_ref(), "59", &mut warnings);

    // :71A: charge bearer
    let charge_bearer = code_to_charges(&tx.chrg_br);

    // Build the field list
    let mut fields: Vec<(String, String)> = vec![
        ("20".into(), reference.clone()),
        ("23B".into(), bank_op_code.into()),
        ("32A".into(), sttlm_field),
        ("50K".into(), ordering_party),
        ("59".into(), beneficiary),
        ("71A".into(), charge_bearer.into()),
    ];

    // :70: remittance info (optional). Wrap to MT 4×35 budget; on
    // overflow, emit the truncation and warn so callers can detect
    // data loss.
    if let Some(rmt) = &tx.rmt_inf {
        if !rmt.ustrd.is_empty() {
            let text: String = rmt
                .ustrd
                .iter()
                .map(|t| t.0.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let wrapped = match wrap_lines(&text, MT_LINE_LEN, MT_PARTY_MAX_LINES) {
                Ok(lines) => lines,
                Err(e) => {
                    warnings.add(
                        ":70:",
                        format!(
                            "remittance info truncated: {} chars dropped (MT :70: max {}x{})",
                            e.overflow_chars, MT_PARTY_MAX_LINES, MT_LINE_LEN
                        ),
                    );
                    e.truncated
                }
            };
            if !wrapped.is_empty() {
                fields.push(("70".into(), wrapped.join("\n")));
            }
        }
    }

    // Sender/receiver BICs are required for the MT application header.
    // Producing a placeholder here would emit a schema-invalid MT message
    // (no valid BIC is 12 characters), so absence is a translation error.
    let sender_bic =
        extract_bic_from_fi(&tx.dbtr_agt).ok_or_else(|| TranslationError::MissingField {
            field: "DbtrAgt/FinInstnId/BICFI".into(),
            context: "pacs008_to_mt103 (block 1 sender)".into(),
        })?;
    let receiver_bic =
        extract_bic_from_fi(&tx.cdtr_agt).ok_or_else(|| TranslationError::MissingField {
            field: "CdtrAgt/FinInstnId/BICFI".into(),
            context: "pacs008_to_mt103 (block 2 receiver)".into(),
        })?;

    let mt_text = format_mt_message("103", &sender_bic, &receiver_bic, &fields);

    Ok(TranslationResult {
        message: mt_text,
        warnings,
    })
}

/// Build an MT party field value from a `PartyIdentification272` and optional
/// `CashAccount40`.
///
/// Format:
/// ```text
/// /IBAN_OR_ACCOUNT   (if account present, on its own line)
/// PARTY NAME (wrapped onto remaining lines, MT 4x35 budget)
/// ```
///
/// The MT :50K: / :59: party fields are limited to 4 lines of 35
/// characters. The account line (`/...`) is hard-truncated if it
/// exceeds the budget; the name is word-wrapped onto whatever lines
/// remain. Anything that does not fit is dropped and a warning is
/// recorded against `field_tag`.
fn build_party_field(
    party: &pacs008::PartyIdentification272,
    acct: Option<&pacs008::CashAccount40>,
    field_tag: &str,
    warnings: &mut TranslationWarnings,
) -> String {
    let mut lines: Vec<String> = Vec::new();

    if let Some(a) = acct {
        if let Some(id) = &a.id {
            let acct_line = match &id.inner {
                pacs008::AccountIdentification4Choice::IBAN(iban) => format!("/{}", iban.0),
                pacs008::AccountIdentification4Choice::Othr(othr) => format!("/{}", othr.id.0),
            };
            if acct_line.len() > MT_LINE_LEN {
                warnings.add(
                    format!(":{field_tag}:"),
                    format!(
                        "account line truncated: {} chars over the MT {}-char limit",
                        acct_line.len() - MT_LINE_LEN,
                        MT_LINE_LEN
                    ),
                );
                lines.push(acct_line[..MT_LINE_LEN].to_string());
            } else {
                lines.push(acct_line);
            }
        }
    }

    let name_budget = MT_PARTY_MAX_LINES.saturating_sub(lines.len());
    if let Some(nm) = &party.nm {
        if name_budget == 0 {
            warnings.add(
                format!(":{field_tag}:"),
                "party name dropped: no MT lines remain after the account line".to_string(),
            );
        } else {
            let wrapped = match wrap_lines(&nm.0, MT_LINE_LEN, name_budget) {
                Ok(l) => l,
                Err(e) => {
                    warnings.add(
                        format!(":{field_tag}:"),
                        format!(
                            "party name truncated: {} chars dropped (max {}x{})",
                            e.overflow_chars, name_budget, MT_LINE_LEN
                        ),
                    );
                    e.truncated
                }
            };
            lines.extend(wrapped);
        }
    }

    lines.join("\n")
}

/// Extract the BIC string from a `BranchAndFinancialInstitutionIdentification8`.
///
/// Returns `None` when neither `BICFI` nor `Nm` is available.
fn extract_bic_from_fi(
    fi: &pacs008::BranchAndFinancialInstitutionIdentification8,
) -> Option<String> {
    fi.fin_instn_id
        .bicfi
        .as_ref()
        .map(|b| b.0.clone())
        .or_else(|| fi.fin_instn_id.nm.as_ref().map(|n| n.0.clone()))
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mappings::mt103_to_pacs008::mt103_to_pacs008;
    use crate::mt::fields::mt103::parse_mt103;
    use crate::mt::parser::parse;

    const MT103_RAW: &str = "\
{1:F01BANKBEBBAXXX0000000000}\
{2:I103BANKDEFFXXXXN}\
{3:{108:MT103REF}}\
{4:
:20:REFERENCE123
:23B:CRED
:32A:230615EUR1000,50
:52A:BANKBEBBAXXX
:50K:/DE89370400440532013000
JOHN DOE
123 MAIN STREET
:57A:BANKDEFFXXXX
:59:/GB29NWBK60161331926819
JANE SMITH
456 HIGH STREET
:71A:SHA
:70:INVOICE 12345
-}{5:{CHK:ABC12345678}}";

    fn roundtrip_doc() -> pacs008::Document {
        let msg = parse(MT103_RAW).unwrap();
        let mt103 = parse_mt103(&msg.block4).unwrap();
        mt103_to_pacs008(&mt103, "MSG001", "2023-06-15T10:00:00")
            .unwrap()
            .message
    }

    #[test]
    fn test_pacs008_to_mt103_contains_reference() {
        let doc = roundtrip_doc();
        let result = pacs008_to_mt103(&doc).unwrap();
        assert!(result.message.contains(":20:REFERENCE123"));
    }

    #[test]
    fn test_pacs008_to_mt103_contains_32a() {
        let doc = roundtrip_doc();
        let result = pacs008_to_mt103(&doc).unwrap();
        assert!(result.message.contains(":32A:230615EUR1000,50"));
    }

    #[test]
    fn test_pacs008_to_mt103_contains_charge_bearer() {
        let doc = roundtrip_doc();
        let result = pacs008_to_mt103(&doc).unwrap();
        assert!(result.message.contains(":71A:SHA"));
    }

    #[test]
    fn test_pacs008_to_mt103_contains_parties() {
        let doc = roundtrip_doc();
        let result = pacs008_to_mt103(&doc).unwrap();
        assert!(result.message.contains("JOHN DOE"));
        assert!(result.message.contains("JANE SMITH"));
    }

    #[test]
    fn test_pacs008_to_mt103_empty_cdt_trf_tx() {
        let doc = pacs008::Document {
            fi_to_fi_cstmr_cdt_trf: pacs008::FIToFICustomerCreditTransferV13 {
                grp_hdr: pacs008::GroupHeader131::builder()
                    .msg_id(pacs008::Max35Text("X".into()))
                    .cre_dt_tm(pacs008::ISODateTime("2023-01-01T00:00:00".into()))
                    .nb_of_txs(pacs008::Max15NumericText("0".into()))
                    .sttlm_inf(pacs008::SettlementInstruction15 {
                        sttlm_mtd: pacs008::SettlementMethod1Code::Inda,
                        sttlm_acct: None,
                        clr_sys: None,
                        instg_rmbrsmnt_agt: None,
                        instg_rmbrsmnt_agt_acct: None,
                        instd_rmbrsmnt_agt: None,
                        instd_rmbrsmnt_agt_acct: None,
                        thrd_rmbrsmnt_agt: None,
                        thrd_rmbrsmnt_agt_acct: None,
                    })
                    .build()
                    .unwrap(),
                cdt_trf_tx_inf: vec![],
                splmtry_data: vec![],
            },
        };
        let err = pacs008_to_mt103(&doc).unwrap_err();
        assert!(matches!(err, TranslationError::MissingField { .. }));
    }

    #[test]
    fn test_pacs008_to_mt103_missing_sender_bic_errors() {
        let mut doc = roundtrip_doc();
        let tx = &mut doc.fi_to_fi_cstmr_cdt_trf.cdt_trf_tx_inf[0];
        tx.dbtr_agt.fin_instn_id.bicfi = None;
        tx.dbtr_agt.fin_instn_id.nm = None;

        let err = pacs008_to_mt103(&doc).unwrap_err();
        match err {
            TranslationError::MissingField { ref field, .. } => {
                assert!(
                    field.starts_with("DbtrAgt/"),
                    "expected DbtrAgt missing-field error, got: {field}"
                );
            }
            _ => panic!("expected MissingField, got: {err}"),
        }
    }

    #[test]
    fn test_pacs008_to_mt103_missing_receiver_bic_errors() {
        let mut doc = roundtrip_doc();
        let tx = &mut doc.fi_to_fi_cstmr_cdt_trf.cdt_trf_tx_inf[0];
        tx.cdtr_agt.fin_instn_id.bicfi = None;
        tx.cdtr_agt.fin_instn_id.nm = None;

        let err = pacs008_to_mt103(&doc).unwrap_err();
        match err {
            TranslationError::MissingField { ref field, .. } => {
                assert!(
                    field.starts_with("CdtrAgt/"),
                    "expected CdtrAgt missing-field error, got: {field}"
                );
            }
            _ => panic!("expected MissingField, got: {err}"),
        }
    }

    #[test]
    fn test_pacs008_to_mt103_wraps_long_party_name() {
        let mut doc = roundtrip_doc();
        let tx = &mut doc.fi_to_fi_cstmr_cdt_trf.cdt_trf_tx_inf[0];
        // Replace the dbtr name with one that exceeds the 35-char single
        // line budget but fits when wrapped onto the remaining 3 lines
        // (account is on line 1).
        tx.dbtr.nm = Some(pacs008::Max140Text(
            "ACME CORPORATION INTERNATIONAL HOLDINGS LIMITED PARTNERSHIP".to_string(),
        ));

        let result = pacs008_to_mt103(&doc).unwrap();
        let block4 = result.message.split("{4:\n").nth(1).unwrap();

        let mut in_50k = false;
        let mut name_lines: Vec<&str> = Vec::new();
        for line in block4.lines() {
            if line.starts_with(":50K:") {
                in_50k = true;
                continue;
            }
            if in_50k {
                if line.starts_with(':') || line.starts_with('-') {
                    break;
                }
                name_lines.push(line);
            }
        }

        assert!(
            !name_lines.is_empty(),
            "expected at least one name line under :50K:"
        );
        for line in &name_lines {
            assert!(
                line.len() <= 35,
                ":50K: name line {:?} exceeds 35 chars",
                line
            );
        }
        let joined: String = name_lines.join(" ");
        assert!(
            joined.contains("ACME CORPORATION") && joined.contains("HOLDINGS"),
            "wrapped name should still spell out the original; got {joined:?}"
        );
        assert!(
            !result
                .warnings
                .warnings
                .iter()
                .any(|w| w.field == ":50K:" && w.message.contains("truncated")),
            "name fits within budget; no :50K: truncation warning expected, got {:?}",
            result.warnings.warnings
        );
    }

    #[test]
    fn test_pacs008_to_mt103_warns_on_party_name_overflow() {
        let mut doc = roundtrip_doc();
        let tx = &mut doc.fi_to_fi_cstmr_cdt_trf.cdt_trf_tx_inf[0];
        // 140 chars of A (the Max140Text limit) cannot fit in 3 lines x
        // 35 chars (105 chars), so wrap_lines must signal overflow.
        tx.dbtr.nm = Some(pacs008::Max140Text("A".repeat(140)));

        let result = pacs008_to_mt103(&doc).unwrap();
        assert!(
            result
                .warnings
                .warnings
                .iter()
                .any(|w| w.field == ":50K:" && w.message.contains("truncated")),
            "expected a :50K: truncation warning, got {:?}",
            result.warnings.warnings
        );
    }

    #[test]
    fn test_pacs008_to_mt103_warns_on_remittance_overflow() {
        let mut doc = roundtrip_doc();
        let tx = &mut doc.fi_to_fi_cstmr_cdt_trf.cdt_trf_tx_inf[0];
        // 5 ustrd lines of 100 chars each → 500 chars; cannot fit in
        // 4 x 35 = 140.
        tx.rmt_inf = Some(pacs008::RemittanceInformation22 {
            ustrd: (0..5)
                .map(|_| pacs008::Max140Text("X".repeat(100)))
                .collect(),
            strd: vec![],
        });

        let result = pacs008_to_mt103(&doc).unwrap();
        assert!(
            result
                .warnings
                .warnings
                .iter()
                .any(|w| w.field == ":70:" && w.message.contains("truncated")),
            "expected a :70: truncation warning, got {:?}",
            result.warnings.warnings
        );
    }
}
