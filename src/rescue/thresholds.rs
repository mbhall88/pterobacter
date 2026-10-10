//! v2 ANI thresholds as exact decimals, and the gates derived from them.
use std::cmp::Ordering;

use serde::Deserialize;

use crate::evidence::{EvidenceError, Percent};

/// A nonnegative exact decimal percentage, `num / den`.
#[derive(Debug, Clone)]
pub(crate) struct Exact {
    num: num_bigint::BigUint,
    den: num_bigint::BigUint,
    scale: usize,
}

impl Exact {
    fn whole(percent: u128) -> Self {
        Self {
            num: percent.into(),
            den: 1u32.into(),
            scale: 0,
        }
    }

    fn parse(text: &str) -> Option<Self> {
        let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
        let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
        if whole.is_empty()
            || !digits(whole)
            || !digits(fraction)
            || (text.contains('.') && fraction.is_empty())
        {
            return None;
        }
        let fraction = fraction.trim_end_matches('0');
        let scale = fraction.len();
        let den = num_bigint::BigUint::from(10u32).pow(u32::try_from(scale).ok()?);
        let num = format!("{whole}{fraction}").parse().ok()?;
        Some(Self { num, den, scale })
    }

    /// Compare with a metric's exact percentage, without a decimal precision limit.
    pub(crate) fn cmp_ratio(&self, num: u128, den: u128) -> Option<Ordering> {
        if den == 0 {
            return None;
        }
        Some(
            (&self.num * num_bigint::BigUint::from(den))
                .cmp(&(num_bigint::BigUint::from(num) * &self.den)),
        )
    }

    /// skani emits hundredths, so comparing with this ceiling is exact for its values.
    pub(crate) fn ceil_hundredths(&self) -> Percent {
        let hundredths = (&self.num * 100u32 + &self.den - 1u32) / &self.den;
        Percent::from_hundredths(u32::try_from(hundredths).expect("threshold at most 100%"))
    }

    /// `self − 1.5`, retaining a power-of-ten denominator. Only called on values ≥ 80.
    fn sub_one_and_a_half(&self) -> Self {
        Self {
            num: &self.num * 10u32 - &self.den * 15u32,
            den: &self.den * 10u32,
            scale: self.scale + 1,
        }
    }

    fn min(self, other: Self) -> Self {
        if &self.num * &other.den > &other.num * &self.den {
            other
        } else {
            self
        }
    }

    fn max(self, other: Self) -> Self {
        if &self.num * &other.den < &other.num * &self.den {
            other
        } else {
            self
        }
    }

    fn display(&self) -> String {
        let text = format!("{:0>width$}", self.num.to_string(), width = self.scale + 1);
        let (whole, fraction) = text.split_at(text.len() - self.scale);
        let fraction = fraction.trim_end_matches('0');
        if fraction.is_empty() {
            whole.to_owned()
        } else {
            format!("{whole}.{fraction}")
        }
    }
}

/// The species (T) and subspecies (S) ANI thresholds for v2.
///
/// The standard values are T = 96 and S = 98. Any explicitly supplied value, even one equal to
/// a standard value, makes the result custom and unvalidated.
#[derive(Debug, Clone)]
pub struct Thresholds {
    species: Exact,
    subspecies: Exact,
    supplied: Option<(Option<String>, Option<String>)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawThresholds {
    species_ani: Option<serde_json::Value>,
    subspecies_ani: Option<serde_json::Value>,
}

fn literal(value: Option<serde_json::Value>) -> Result<Option<String>, EvidenceError> {
    match value {
        None => Ok(None),
        Some(serde_json::Value::String(s)) => Ok(Some(s)),
        Some(serde_json::Value::Number(n)) => Ok(Some(n.to_string())),
        Some(_) => Err(EvidenceError::Settings(
            "a threshold must be a decimal number".into(),
        )),
    }
}

impl Thresholds {
    pub fn standard() -> Self {
        Thresholds {
            species: Exact::whole(96),
            subspecies: Exact::whole(98),
            supplied: None,
        }
    }

    /// Explicit overrides. Each must be a finite decimal with 80 ≤ T ≤ S ≤ 100.
    pub fn custom(species: Option<&str>, subspecies: Option<&str>) -> Result<Self, EvidenceError> {
        if species.is_none() && subspecies.is_none() {
            return Err(EvidenceError::Settings(
                "custom thresholds need a species or subspecies ANI".into(),
            ));
        }
        let parse = |text: Option<&str>, default: Exact| -> Result<Exact, EvidenceError> {
            text.map_or(Ok(default), |t| {
                Exact::parse(t).ok_or_else(|| {
                    EvidenceError::Settings(format!("{t} is not a finite decimal percentage"))
                })
            })
        };
        let standard = Self::standard();
        let species_ani = parse(species, standard.species)?;
        let subspecies_ani = parse(subspecies, standard.subspecies)?;
        let in_order = |a: &Exact, b: &Exact| &a.num * &b.den <= &b.num * &a.den;
        if !(in_order(&Exact::whole(80), &species_ani)
            && in_order(&species_ani, &subspecies_ani)
            && in_order(&subspecies_ani, &Exact::whole(100)))
        {
            return Err(EvidenceError::Settings(
                "thresholds must satisfy 80 <= species ANI <= subspecies ANI <= 100".into(),
            ));
        }
        Ok(Thresholds {
            species: species_ani,
            subspecies: subspecies_ani,
            supplied: Some((species.map(str::to_owned), subspecies.map(str::to_owned))),
        })
    }

    /// Overrides as JSON: `{"species_ani": "95.5", "subspecies_ani": "98"}`. Any other key,
    /// such as an AF override, is refused.
    pub fn from_json(text: &str) -> Result<Self, EvidenceError> {
        let raw: RawThresholds = serde_json::from_str(text)?;
        let species = literal(raw.species_ani)?;
        let subspecies = literal(raw.subspecies_ani)?;
        Self::custom(species.as_deref(), subspecies.as_deref())
    }

    pub fn is_custom(&self) -> bool {
        self.supplied.is_some()
    }

    /// Policy settings and their canonical digest, available before any evidence is read.
    pub fn policy_value(&self) -> serde_json::Value {
        use super::{CONTRACT_REVISION, CUSTOM_UNVALIDATED, NOT_YET_VALIDATED, V2_POLICY_ID};
        use serde_json::json;
        let custom = self.is_custom();
        let mut policy = json!({"policy_id": V2_POLICY_ID, "contract_revision": CONTRACT_REVISION,
            "policy_mode": if custom { "custom" } else { "default_candidate" },
            "validation_status": if custom { CUSTOM_UNVALIDATED } else { NOT_YET_VALIDATED },
            "thresholds": self.to_json()});
        // Hash effective settings and mode, retaining literal flags separately in the report.
        let mut canonical = policy.clone();
        canonical["thresholds"]
            .as_object_mut()
            .expect("threshold object")
            .remove("supplied");
        policy["settings_sha256"] = json!(crate::evidence::sha256_hex(
            canonical.to_string().as_bytes()
        ));
        policy
    }

    pub(crate) fn species(&self) -> &Exact {
        &self.species
    }

    pub(crate) fn subspecies(&self) -> &Exact {
        &self.subspecies
    }

    /// Lower trigger endpoint, max(80, T − 1.5).
    pub(crate) fn trigger_lower(&self) -> Exact {
        self.species.sub_one_and_a_half().max(Exact::whole(80))
    }

    /// The conflict ANI for the two-parent alert and foreign-parent challenge, min(96, T).
    pub(crate) fn conflict(&self) -> Exact {
        self.species.clone().min(Exact::whole(96))
    }

    pub(crate) fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "species_ani": self.species.display(),
            "subspecies_ani": self.subspecies.display(),
            "trigger_lower_ani": self.trigger_lower().display(),
            "conflict_ani": self.conflict().display(),
            "both_af_minimum": "80",
            "subspecies_margin_percentage_points": "1",
            "supplied": self.supplied.as_ref().map(|(s, ss)| serde_json::json!({
                "species_ani": s, "subspecies_ani": ss})),
        })
    }
}
