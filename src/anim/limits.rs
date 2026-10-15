//! Versioned execution controls. Numerical ANIm and decision identities are separate.
use super::AnimError;
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

pub const EXECUTION_PROFILE: &str = "terobacter.anim-execution/2";
pub type Restrictions = BTreeMap<String, Restriction>;

fn invalid(message: &str) -> AnimError {
    AnimError::Invalid(message.into())
}

// Option fields in a new profile must be present, even when their value is null.
pub(crate) fn required_option<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Restriction {
    #[serde(deserialize_with = "required_option")]
    pub soft: Option<u64>,
    #[serde(deserialize_with = "required_option")]
    pub hard: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Execution {
    pub profile: String,
    #[serde(deserialize_with = "required_option")]
    pub configured_pair_timeout: Option<Duration>,
    #[serde(deserialize_with = "required_option")]
    pub remaining_query_budget: Option<Duration>,
    #[serde(deserialize_with = "required_option")]
    pub effective_pair_timeout: Option<Duration>,
    #[serde(deserialize_with = "required_option")]
    pub memory_mib: Option<u64>,
    pub inherited: Restrictions,
}

pub fn minimum(a: Option<Duration>, b: Option<Duration>) -> Option<Duration> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

impl Execution {
    pub fn new(
        pair: Option<Duration>,
        remaining: Option<Duration>,
        memory: Option<u64>,
        inherited: Restrictions,
    ) -> Result<Self, AnimError> {
        let value = Self {
            profile: EXECUTION_PROFILE.into(),
            configured_pair_timeout: pair,
            remaining_query_budget: remaining,
            effective_pair_timeout: minimum(pair, remaining),
            memory_mib: memory,
            inherited,
        };
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<(), AnimError> {
        if self.profile != EXECUTION_PROFILE
            || self.effective_pair_timeout
                != minimum(self.configured_pair_timeout, self.remaining_query_budget)
        {
            return Err(invalid(
                "unknown execution profile or inconsistent effective pair timeout",
            ));
        }
        for limit in [
            self.configured_pair_timeout,
            self.remaining_query_budget,
            self.effective_pair_timeout,
        ]
        .into_iter()
        .flatten()
        {
            validate_duration(limit).map_err(|e| invalid(&e))?;
        }
        if let Some(memory) = self.memory_mib {
            memory_bytes(memory).map_err(|e| invalid(&e))?;
        }
        if self
            .inherited
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>()
            != [
                "address_space",
                "cpu",
                "data",
                "file_size",
                "resident_set",
                "stack",
            ]
        {
            return Err(invalid("missing or unknown inherited restrictions"));
        }
        for limit in self.inherited.values() {
            for n in [limit.soft, limit.hard].into_iter().flatten() {
                finite_rlim(n).map_err(|e| invalid(&e))?;
            }
            if limit
                .hard
                .is_some_and(|hard| limit.soft.is_none_or(|soft| soft > hard))
            {
                return Err(invalid("inherited soft limit exceeds hard limit"));
            }
        }
        Ok(())
    }
}

pub fn validate_duration(duration: Duration) -> Result<(), String> {
    if duration.is_zero() || Instant::now().checked_add(duration).is_none() {
        return Err("time limit must be positive and representable as a deadline".into());
    }
    cpu_seconds(duration)?;
    Ok(())
}
pub fn cpu_seconds(duration: Duration) -> Result<u64, String> {
    let n = duration
        .as_secs()
        .checked_add(u64::from(duration.subsec_nanos() != 0))
        .ok_or("CPU limit overflow")?;
    finite_rlim(n)?;
    Ok(n)
}
pub fn memory_bytes(mib: u64) -> Result<u64, String> {
    let n = mib
        .checked_mul(1024 * 1024)
        .filter(|n| *n > 0)
        .ok_or("memory MiB conversion overflow or zero")?;
    finite_rlim(n)?;
    Ok(n)
}
fn finite_rlim(n: u64) -> Result<libc::rlim_t, String> {
    let value = libc::rlim_t::try_from(n).map_err(|_| "limit does not fit platform rlim_t")?;
    #[cfg(target_os = "linux")]
    let sentinel = value == libc::RLIM_INFINITY;
    #[cfg(target_os = "macos")]
    let sentinel = value >= libc::RLIM_INFINITY;
    if sentinel {
        return Err("finite limit reaches RLIM_INFINITY".into());
    }
    Ok(value)
}
#[cfg(target_os = "linux")]
type Resource = libc::__rlimit_resource_t;
#[cfg(target_os = "macos")]
type Resource = libc::c_int;

pub fn inherited() -> Result<Restrictions, AnimError> {
    let mut result = BTreeMap::new();
    for (name, resource) in [
        ("cpu", libc::RLIMIT_CPU),
        ("address_space", libc::RLIMIT_AS),
        ("data", libc::RLIMIT_DATA),
        ("resident_set", libc::RLIMIT_RSS),
        ("stack", libc::RLIMIT_STACK),
        ("file_size", libc::RLIMIT_FSIZE),
    ] {
        let mut value = std::mem::MaybeUninit::<libc::rlimit>::uninit();
        if unsafe { libc::getrlimit(resource, value.as_mut_ptr()) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let value = unsafe { value.assume_init() };
        let decode = |n: libc::rlim_t| {
            if n == libc::RLIM_INFINITY {
                None
            } else {
                Some(n)
            }
        };
        result.insert(
            name.into(),
            Restriction {
                soft: decode(value.rlim_cur),
                hard: decode(value.rlim_max),
            },
        );
    }
    Ok(result)
}
pub fn cap(inherited: &Restriction, limit: u64) -> Restriction {
    Restriction {
        soft: Some(inherited.soft.map_or(limit, |n| n.min(limit))),
        hard: Some(inherited.hard.map_or(limit, |n| n.min(limit))),
    }
}
fn set(resource: Resource, value: &Restriction) -> Result<(), AnimError> {
    let encode = |n: Option<u64>| -> Result<libc::rlim_t, AnimError> {
        n.map(finite_rlim)
            .transpose()
            .map(|n| n.unwrap_or(libc::RLIM_INFINITY))
            .map_err(|e| invalid(&e))
    };
    let value = libc::rlimit {
        rlim_cur: encode(value.soft)?,
        rlim_max: encode(value.hard)?,
    };
    if unsafe { libc::setrlimit(resource, &value) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerExecution {
    pub pair: Execution,
    pub pair_elapsed: Duration,
    #[serde(deserialize_with = "required_option")]
    pub orientation_timeout: Option<Duration>,
}
impl WorkerExecution {
    pub fn validate(&self) -> Result<(), AnimError> {
        self.pair.validate()?;
        if self.orientation_timeout
            != self
                .pair
                .effective_pair_timeout
                .map(|time| time.saturating_sub(self.pair_elapsed))
        {
            return Err(invalid(
                "orientation timeout is inconsistent with elapsed pair time",
            ));
        }
        if let Some(time) = self.orientation_timeout {
            validate_duration(time).map_err(|e| invalid(&e))?;
        }
        Ok(())
    }
    pub fn limits(&self) -> Result<WorkerLimits, AnimError> {
        self.validate()?;
        let mut effective = self.pair.inherited.clone();
        let cpu = self
            .orientation_timeout
            .map(cpu_seconds)
            .transpose()
            .map_err(|e| invalid(&e))?;
        if let Some(n) = cpu {
            effective.insert("cpu".into(), cap(&effective["cpu"], n));
        }
        #[cfg(target_os = "linux")]
        if let Some(mib) = self.pair.memory_mib {
            effective.insert(
                "address_space".into(),
                cap(
                    &effective["address_space"],
                    memory_bytes(mib).map_err(|e| invalid(&e))?,
                ),
            );
        }
        Ok(WorkerLimits {
            request: self.clone(),
            effective,
            tool_cpu_seconds: cpu,
            memory_enforcement: if cfg!(target_os = "linux") {
                "optional_RLIMIT_AS"
            } else {
                "optional_Mach_peak_RSS_10ms"
            }
            .into(),
        })
    }
    pub fn install(&self) -> Result<WorkerLimits, AnimError> {
        let limits = self.limits()?;
        if inherited()? != self.pair.inherited {
            return Err(invalid("worker inherited restrictions changed"));
        }
        if self.orientation_timeout.is_some() {
            set(libc::RLIMIT_CPU, &limits.effective["cpu"])?;
        }
        #[cfg(target_os = "linux")]
        if self.pair.memory_mib.is_some() {
            set(libc::RLIMIT_AS, &limits.effective["address_space"])?;
        }
        if inherited()? != limits.effective {
            return Err(invalid("effective worker restrictions differ"));
        }
        Ok(limits)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerLimits {
    pub request: WorkerExecution,
    pub effective: Restrictions,
    #[serde(deserialize_with = "required_option")]
    pub tool_cpu_seconds: Option<u64>,
    pub memory_enforcement: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numeric_boundaries_and_componentwise_inheritance() {
        assert!(validate_duration(Duration::MAX).is_err());
        assert!(finite_rlim(libc::RLIM_INFINITY).is_err());
        assert!(memory_bytes(u64::MAX).is_err());
        assert_eq!(
            cap(
                &Restriction {
                    soft: Some(2),
                    hard: Some(20)
                },
                10
            ),
            Restriction {
                soft: Some(2),
                hard: Some(10)
            }
        );
        assert_eq!(
            cap(
                &Restriction {
                    soft: None,
                    hard: None
                },
                10
            ),
            Restriction {
                soft: Some(10),
                hard: Some(10)
            }
        );
        assert_eq!(
            minimum(None, Some(Duration::from_secs(2))),
            Some(Duration::from_secs(2))
        );
    }
    #[test]
    fn new_fields_must_be_explicit_even_when_unlimited() {
        let execution = Execution::new(None, None, None, inherited().unwrap()).unwrap();
        let mut json = serde_json::to_value(&execution).unwrap();
        json.as_object_mut().unwrap().remove("memory_mib");
        assert!(serde_json::from_value::<Execution>(json).is_err());
        let mut execution = execution;
        execution.profile = "unknown".into();
        assert!(execution.validate().is_err());
    }
}
