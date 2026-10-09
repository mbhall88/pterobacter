//! Binding saved evidence to a verified package: the evidence must have been computed against
//! this package, and every reference it names must be one the package holds with the same taxa.

use crate::evidence::{Reference, SavedEvidence};

use super::{PackageError, PackageReference, VerifiedPackage};

impl VerifiedPackage {
    pub fn check_evidence(&self, evidence: &SavedEvidence) -> Result<(), PackageError> {
        let provenance = evidence.provenance();
        if provenance.reference_package_sha256 != self.package.manifest_sha256 {
            return Err(unresolved(
                "the evidence was computed against another reference package",
            ));
        }
        let component = self
            .component(&provenance.reference_component_id)
            .ok_or_else(|| {
                unresolved(format!(
                    "the package has no component {}",
                    provenance.reference_component_id
                ))
            })?;
        for reference in evidence.references() {
            let packaged = component
                .references
                .iter()
                .find(|packaged| packaged.reference_id == reference.id)
                .ok_or_else(|| {
                    unresolved(format!(
                        "{} is not a reference of the package",
                        reference.id
                    ))
                })?;
            check_taxa(reference, packaged)?;
        }
        Ok(())
    }
}

fn check_taxa(reference: &Reference, packaged: &PackageReference) -> Result<(), PackageError> {
    let species = reference
        .species
        .as_ref()
        .map_or("", |taxon| taxon.id.as_str());
    let subspecies = reference.subspecies.as_ref().map(|taxon| taxon.id.as_str());
    if species != packaged.species_id || subspecies != packaged.subspecies_id.as_deref() {
        return Err(unresolved(format!(
            "{} is assigned to other taxa than the package gives it",
            reference.id
        )));
    }
    Ok(())
}

fn unresolved(message: impl Into<String>) -> PackageError {
    PackageError::Unresolved(message.into())
}
