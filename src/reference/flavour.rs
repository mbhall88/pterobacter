//! Exact metadata profiles; neither profile changes scientific verification.
use super::{inconsistent, PackageError};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Flavour {
    Development,
    Public,
}
impl Flavour {
    pub fn from_schema(schema: &str) -> Result<Self, PackageError> {
        match schema {
            "2.0.0-dev-portable-reference-package" => Ok(Self::Development),
            "2.1.0-public-portable-reference-package" => Ok(Self::Public),
            _ => Err(PackageError::Incompatible(format!(
                "unsupported portable schema {schema:?}"
            ))),
        }
    }
    pub fn source_schema(self) -> &'static str {
        match self {
            Self::Development => "1.0.0-dev-reference-package",
            Self::Public => "1.1.0-public-reference-package",
        }
    }
    pub fn release_state(self) -> &'static str {
        match self {
            Self::Development => "development_proposal_not_distributable",
            Self::Public => "public_reference_export",
        }
    }
    pub fn trust_basis(self) -> &'static str {
        match self {
            Self::Development => {
                "expected_package_digest_and_export_attestation_not_authenticated_release"
            }
            Self::Public => "catalogue_pinned_manifest_digest_and_export_attestation",
        }
    }
    pub fn path_semantics(self) -> &'static str {
        match self {
            Self::Development => "absolute_existing_local_sources_no_relocation_or_sequence_copy",
            Self::Public => "namespace_labels_not_local_paths",
        }
    }
    pub fn export_scope(self) -> &'static str {
        match self {
            Self::Development => "full_local_source_validation_at_export_not_reexecuted_at_load",
            Self::Public => {
                "frozen_package_transformation_validated_at_export_not_reexecuted_at_load"
            }
        }
    }
    pub fn no_external_distribution(self) -> bool {
        self == Self::Development
    }
    pub fn check(self, state: &str, trust: &str) -> Result<(), PackageError> {
        if state != self.release_state() || trust != self.trust_basis() {
            return Err(inconsistent(
                "mixed portable schema, release state or trust basis",
            ));
        }
        Ok(())
    }
}
