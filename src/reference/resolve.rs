//! Binding saved evidence to a verified package. The evidence must have been computed against
//! this package, over the whole component it names, and every reference row must say exactly what
//! the package says about that reference: its taxa and their display names, its name status and
//! its role. Contradictory evidence is refused, never rewritten.

use crate::evidence::{Reference, SavedEvidence, Taxon};

use super::{Concept, PackageError, PackageReference, ReferenceComponent, VerifiedPackage};

impl VerifiedPackage {
    pub fn check_evidence(&self, evidence: &SavedEvidence) -> Result<(), PackageError> {
        let provenance = evidence.provenance();
        let digest = provenance.reference_package_sha256.as_str();
        let identity = &self.package;
        if digest != identity.manifest_sha256
            && identity.converted_from_sha256.as_deref() != Some(digest)
        {
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
        if provenance.engine_version != component.engine_version {
            return Err(unresolved(format!(
                "the evidence was computed with {} {}, but component {} was built with {}",
                component.engine, provenance.engine_version, component.id, component.engine_version
            )));
        }
        if evidence.reference_count() != component.references.len() {
            return Err(unresolved(format!(
                "the evidence counts {} references, but component {} holds {}",
                evidence.reference_count(),
                component.id,
                component.references.len()
            )));
        }
        for reference in evidence.references() {
            let packaged = component.reference(&reference.id).ok_or_else(|| {
                unresolved(format!(
                    "{} is not a reference of component {}",
                    reference.id, component.id
                ))
            })?;
            self.check_reference(reference, packaged, component)?;
        }
        Ok(())
    }

    /// The row the package would write for `packaged`, field by field.
    fn check_reference(
        &self,
        reference: &Reference,
        packaged: &PackageReference,
        component: &ReferenceComponent,
    ) -> Result<(), PackageError> {
        let id = &reference.id;
        let concept = |concept_id: &str| {
            self.concept(concept_id).ok_or_else(|| {
                unresolved(format!(
                    "{id}: the package names {concept_id}, which is not one of its concepts"
                ))
            })
        };
        let species = concept(&packaged.species_id)?;
        check_taxon(id, "species", reference.species.as_ref(), Some(species))?;
        let subspecies = packaged.subspecies_id.as_deref().map(concept).transpose()?;
        check_taxon(id, "subspecies", reference.subspecies.as_ref(), subspecies)?;
        if let Some(lineage) = &reference.lineage_id {
            return Err(unresolved(format!(
                "{id}: the evidence gives lineage {lineage}, but a reference of component {} has none",
                component.id
            )));
        }
        let status = &subspecies.unwrap_or(species).nomenclature_status;
        if reference.name_status != *status {
            return Err(unresolved(format!(
                "{id}: the evidence gives name status {}, the package gives {status}",
                reference.name_status
            )));
        }
        let role = if packaged.type_anchor {
            "type_anchor"
        } else {
            "diversity"
        };
        if reference.reference_role != role {
            return Err(unresolved(format!(
                "{id}: the evidence gives role {}, the package gives {role}",
                reference.reference_role
            )));
        }
        Ok(())
    }
}

fn check_taxon(
    id: &str,
    rank: &str,
    given: Option<&Taxon>,
    packaged: Option<&Concept>,
) -> Result<(), PackageError> {
    let describe = |taxon: Option<(&str, &str)>| {
        taxon.map_or_else(|| "none".to_string(), |(id, name)| format!("{id} ({name})"))
    };
    let given = given.map(|taxon| (taxon.id.as_str(), taxon.name.as_str()));
    let packaged = packaged.map(|concept| (concept.id.as_str(), concept.display_name.as_str()));
    if given != packaged {
        return Err(unresolved(format!(
            "{id}: the evidence gives {rank} {}, the package gives {}",
            describe(given),
            describe(packaged)
        )));
    }
    Ok(())
}

fn unresolved(message: impl Into<String>) -> PackageError {
    PackageError::Unresolved(message.into())
}
