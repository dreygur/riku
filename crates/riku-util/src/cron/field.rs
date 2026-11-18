//! One cron field parsed into a bitmask: bit `n` set means value `n` is
//! allowed.
//!
//! Accepted syntax, per field: `*`, `n`, `a-b`, `*/s`, `a-b/s` and comma
//! lists of those. Months and weekdays also take three letter names in any
//! case (`jan`, `Mon`). A bare `n/s` is refused, as croner refused it.

#![allow(clippy::question_mark)]

use anyhow::{anyhow, Result};

const MONTH_NAMES: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];
const WEEKDAY_NAMES: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

/// The legal values of one field. `names[i]` stands for `min + i`.
pub(super) struct FieldSpec {
    min: u8,
    max: u8,
    names: &'static [&'static str],
}

pub(super) const MINUTE: FieldSpec = FieldSpec {
    min: 0,
    max: 59,
    names: &[],
};
pub(super) const HOUR: FieldSpec = FieldSpec {
    min: 0,
    max: 23,
    names: &[],
};
pub(super) const DAY: FieldSpec = FieldSpec {
    min: 1,
    max: 31,
    names: &[],
};
pub(super) const MONTH: FieldSpec = FieldSpec {
    min: 1,
    max: 12,
    names: &MONTH_NAMES,
};
/// 0 and 7 are both Sunday; the schedule folds 7 into 0.
pub(super) const WEEKDAY: FieldSpec = FieldSpec {
    min: 0,
    max: 7,
    names: &WEEKDAY_NAMES,
};

fn parse_value(text: &str, spec: &FieldSpec) -> Result<u8> {
    let named = spec
        .names
        .iter()
        .position(|name| name.eq_ignore_ascii_case(text));
    let value = match (named, text.parse::<u8>()) {
        (Some(index), _) => spec.min + index as u8,
        (None, Ok(number)) => number,
        (None, Err(_)) => return Err(anyhow!("'{text}' is not a number")),
    };

    if value < spec.min || value > spec.max {
        return Err(anyhow!("{value} is outside {}-{}", spec.min, spec.max));
    }
    Ok(value)
}

fn parse_step(text: &str) -> Result<usize> {
    match text.parse::<usize>() {
        Ok(0) => Err(anyhow!("step must be at least 1")),
        Ok(step) => Ok(step),
        Err(_) => Err(anyhow!("'{text}' is not a step")),
    }
}

fn parse_bounds(text: &str, spec: &FieldSpec) -> Result<(u8, u8)> {
    if text == "*" {
        return Ok((spec.min, spec.max));
    }
    let Some((low, high)) = text.split_once('-') else {
        let value = match parse_value(text, spec) {
            Ok(value) => value,
            Err(e) => return Err(e),
        };
        return Ok((value, value));
    };

    let low = match parse_value(low, spec) {
        Ok(value) => value,
        Err(e) => return Err(e),
    };
    let high = match parse_value(high, spec) {
        Ok(value) => value,
        Err(e) => return Err(e),
    };
    if low > high {
        return Err(anyhow!("range {text} runs backwards"));
    }
    Ok((low, high))
}

fn parse_part(part: &str, spec: &FieldSpec) -> Result<u64> {
    let (range, step) = match part.split_once('/') {
        Some((range, step)) => (range, Some(step)),
        None => (part, None),
    };
    let step = match step.map(parse_step).transpose() {
        Ok(step) => step,
        Err(e) => return Err(e),
    };
    if step.is_some() && range != "*" && !range.contains('-') {
        return Err(anyhow!("'{part}' needs a range or * before the step"));
    }

    let (low, high) = match parse_bounds(range, spec) {
        Ok(bounds) => bounds,
        Err(e) => return Err(e),
    };
    Ok((low..=high)
        .step_by(step.unwrap_or(1))
        .fold(0, |mask, value| mask | 1 << value))
}

/// The bitmask of the values `text` allows within `spec`.
pub(super) fn parse_field(text: &str, spec: &FieldSpec) -> Result<u64> {
    text.split(',').try_fold(0, |mask, part| {
        if part.is_empty() {
            return Err(anyhow!("empty entry in '{text}'"));
        }
        match parse_part(part, spec) {
            Ok(bits) => Ok(mask | bits),
            Err(e) => Err(anyhow!("'{text}': {e}")),
        }
    })
}
