//! v2 ANI thresholds as exact decimals, and the gates derived from them.
use std::cmp::Ordering;

use serde::Deserialize;

use crate::evidence::{EvidenceError, Percent};

/// More fractional digits than this are refused, which keeps every gate within checked u128.
const MAX_FRACTION_DIGITS: usize = 12;

/// A nonnegative exact decimal percentage, `num / den`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Exact {
    num: u128,
    den: u128,
}

impl Exact {
    const fn whole(percent: u128) -> Self {
        Exact {
            num: percent,
            den: 1,
        }
    }

    fn parse(text: &str) -> Option<Self> {
        let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
        let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
        if whole.is_empty()
            || whole.len() > 3
            || !digits(whole)
            || !digits(fraction)
            || (text.contains('.') && fraction.is_empty())
            || fraction.len() > MAX_FRACTION_DIGITS
        {
            return None;
        }
        let den = 10u128.pow(u32::try_from(fraction.len()).ok()?);
        let num = format!("{whole}{fraction}").parse().ok()?;
        Some(Exact { num, den })
    }

    /// Compare `self` with the rational `num / den`, both percentages.
    pub(crate) fn cmp_ratio(self, num: u128, den: u128) -> Option<Ordering> {
        Some(self.num.checked_mul(den)?.cmp(&num.checked_mul(self.den)?))
    }

    /// The smallest two-decimal percentage at or above this value. skani emits two decimals,
    /// so `ani >= self` is exactly `ani >= self.ceil_hundredths()` for every emitted value.
    pub(crate) fn ceil_hundredths(self) -> Percent {
        let hundredths = (self.num * 100).div_ceil(self.den);
        Percent::from_hundredths(u32::try_from(hundredths).expect("threshold at most 100%"))
    }

    /// `self − 1.5`, keeping a power-of-ten denominator. Only called on values ≥ 80.
    fn sub_one_and_a_half(self) -> Self {
        Exact {
            num: 10 * self.num - 15 * self.den,
            den: 10 * self.den,
        }
    }

    fn min(self, other: Self) -> Self {
        if self.cmp_ratio(other.num, other.den) == Some(Ordering::Greater) {
            other
        } else {
            self
        }
    }

    fn max(self, other: Self) -> Self {
        if self.cmp_ratio(other.num, other.den) == Some(Ordering::Less) {
            other
        } else {
            self
        }
    }

    fn display(self) -> String {
        let scale = self.den.ilog10() as usize;
        let text = format!("{:0>width$}", self.num, width = scale + 1);
        let (whole, fraction) = text.split_at(text.len() - scale);
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
                    EvidenceError::Settings(format!(
                        "{t} is not a finite decimal percentage with at most \
                         {MAX_FRACTION_DIGITS} decimals"
                    ))
                })
            })
        };
        let standard = Self::standard();
        let species_ani = parse(species, standard.species)?;
        let subspecies_ani = parse(subspecies, standard.subspecies)?;
        let in_order = |a: Exact, b: Exact| a.cmp_ratio(b.num, b.den) != Some(Ordering::Greater);
        if !(in_order(Exact::whole(80), species_ani)
            && in_order(species_ani, subspecies_ani)
            && in_order(subspecies_ani, Exact::whole(100)))
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

    pub(crate) fn species(&self) -> Exact {
        self.species
    }

    pub(crate) fn subspecies(&self) -> Exact {
        self.subspecies
    }

    /// Lower trigger endpoint, max(80, T − 1.5).
    pub(crate) fn trigger_lower(&self) -> Exact {
        self.species.sub_one_and_a_half().max(Exact::whole(80))
    }

    /// The conflict ANI for the two-parent alert and foreign-parent challenge, min(96, T).
    pub(crate) fn conflict(&self) -> Exact {
        self.species.min(Exact::whole(96))
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
