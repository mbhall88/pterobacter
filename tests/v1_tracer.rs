//! Decision seam: a frozen 0.2.0 saved-evidence fixture, passed through the
//! internal decision interface, yields its frozen v1 species call as JSON.
//!
//! The expected call comes from the frozen 0.2.0 report (`frozen-v1-call.json`),
//! not from re-running this implementation's own logic.

use serde_json::Value;
use terobacter::decision::{decide, Policy};
use terobacter::evidence::SavedEvidence;

const EVIDENCE: &str = include_str!("fixtures/v1-species-assigned/evidence.json");
const FROZEN_CALL: &str = include_str!("fixtures/v1-species-assigned/frozen-v1-call.json");

fn decision_json(evidence: &str) -> Value {
    let evidence = SavedEvidence::from_json(evidence).expect("fixture evidence should load");
    serde_json::from_str(&decide(&evidence, &Policy::v1()).to_json()).expect("decision is JSON")
}

#[test]
fn fixture_reproduces_its_frozen_v1_species_call() {
    let frozen: Value = serde_json::from_str(FROZEN_CALL).unwrap();

    let decision = decision_json(EVIDENCE);

    assert_eq!(decision["policy"]["id"], frozen["policy"]);
    assert_eq!(decision["species"]["status"], frozen["status"]);
    assert_eq!(decision["species"]["reason"], frozen["reason"]);
    assert_eq!(decision["species"]["id"], frozen["species_id"]);
    assert_eq!(decision["species"]["name"], frozen["species_name"]);
}
