//! Decision seam: a frozen 0.2.0 saved-evidence fixture, passed through the internal decision
//! interface, yields its frozen v1 species call as JSON.

mod common;

#[test]
fn fixture_reproduces_its_frozen_v1_species_call() {
    let decision = common::decision_from_text(common::FIXTURE);

    common::assert_matches_frozen_call(&decision);
}
