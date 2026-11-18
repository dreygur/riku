//! A parsed 5 field schedule and the search for its next fire time in UTC.

#![allow(clippy::question_mark)]

use std::str::FromStr;

use anyhow::{anyhow, Error, Result};
use jiff::civil::{Date, DateTime};
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};

use super::field::{parse_field, FieldSpec, DAY, HOUR, MINUTE, MONTH, WEEKDAY};

/// How far ahead the search looks. February 29 can be eight years apart
/// (2096 to 2104), so a shorter horizon would call it unsatisfiable.
const HORIZON_YEARS: i16 = 8;
const SUNDAY_SEVEN: u64 = 1 << 7;
const FIELDS: [&FieldSpec; 5] = [&MINUTE, &HOUR, &DAY, &MONTH, &WEEKDAY];

/// How day of month and day of week combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DayRule {
    /// At least one field is `*`: a day must pass both.
    Both,
    /// Both fields are restricted: Vixie cron fires when either passes.
    Either,
}

/// The allowed values of each field as bitmasks (bit `n` is value `n`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Schedule {
    minutes: u64,
    hours: u64,
    days: u64,
    months: u64,
    weekdays: u64,
    day_rule: DayRule,
}

fn has(mask: u64, value: i8) -> bool {
    mask & 1 << value != 0
}

/// The lowest set bit at or above `from`.
fn first_from(mask: u64, from: i8) -> Option<i8> {
    let rest = mask & u64::MAX.checked_shl(from as u32).unwrap_or(0);
    if rest == 0 {
        return None;
    }
    Some(rest.trailing_zeros() as i8)
}

fn fold_sunday(mask: u64) -> u64 {
    if mask & SUNDAY_SEVEN == 0 {
        return mask;
    }
    (mask | 1) & !SUNDAY_SEVEN
}

impl FromStr for Schedule {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self> {
        let fields: Vec<&str> = text.split_whitespace().collect();
        let [_, _, day, _, weekday] = fields.as_slice() else {
            return Err(anyhow!("expected 5 fields, got {}", fields.len()));
        };

        let mut masks = [0; 5];
        for ((mask, text), spec) in masks.iter_mut().zip(&fields).zip(FIELDS) {
            *mask = match parse_field(text, spec) {
                Ok(bits) => bits,
                Err(e) => return Err(e),
            };
        }
        let [minutes, hours, days, months, weekdays] = masks;

        let day_rule = if *day == "*" || *weekday == "*" {
            DayRule::Both
        } else {
            DayRule::Either
        };
        Ok(Schedule {
            minutes,
            hours,
            days,
            months,
            weekdays: fold_sunday(weekdays),
            day_rule,
        })
    }
}

impl Schedule {
    fn day_matches(&self, date: Date) -> bool {
        let by_day = has(self.days, date.day());
        let by_weekday = has(self.weekdays, date.weekday().to_sunday_zero_offset());
        match self.day_rule {
            DayRule::Both => by_day && by_weekday,
            DayRule::Either => by_day || by_weekday,
        }
    }

    /// The first allowed hour and minute at or after `hour:minute`.
    fn first_time_from(&self, hour: i8, minute: i8) -> Option<(i8, i8)> {
        if has(self.hours, hour) {
            if let Some(minute) = first_from(self.minutes, minute) {
                return Some((hour, minute));
            }
        }
        let Some(hour) = first_from(self.hours, hour + 1) else {
            return None;
        };
        first_from(self.minutes, 0).map(|minute| (hour, minute))
    }

    /// The first instant at or after `start` (a whole minute) that matches.
    fn first_match_from(&self, start: DateTime) -> Result<Option<DateTime>> {
        let horizon = match start.date().checked_add(HORIZON_YEARS.years()) {
            Ok(horizon) => horizon,
            Err(e) => return Err(anyhow!("search horizon out of range: {e}")),
        };

        let mut date = start.date();
        let (mut hour, mut minute) = (start.hour(), start.minute());
        while date <= horizon {
            if !has(self.months, date.month()) {
                date = match date.last_of_month().tomorrow() {
                    Ok(next) => next,
                    Err(e) => return Err(anyhow!("date out of range: {e}")),
                };
                (hour, minute) = (0, 0);
                continue;
            }
            if self.day_matches(date) {
                if let Some((hour, minute)) = self.first_time_from(hour, minute) {
                    return Ok(Some(date.at(hour, minute, 0, 0)));
                }
            }
            date = match date.tomorrow() {
                Ok(next) => next,
                Err(e) => return Err(anyhow!("date out of range: {e}")),
            };
            (hour, minute) = (0, 0);
        }
        Ok(None)
    }

    /// The next fire time strictly after `after`, or an error when none
    /// falls within the search horizon.
    pub(super) fn next_after(&self, after: Timestamp) -> Result<Timestamp> {
        let next_minute = (after.as_second().div_euclid(60) + 1) * 60;
        let start = match Timestamp::from_second(next_minute) {
            Ok(start) => TimeZone::UTC.to_datetime(start),
            Err(e) => return Err(anyhow!("time out of range: {e}")),
        };

        let found = match self.first_match_from(start) {
            Ok(Some(found)) => found,
            Ok(None) => return Err(anyhow!("no run within {HORIZON_YEARS} years")),
            Err(e) => return Err(e),
        };
        match TimeZone::UTC.to_timestamp(found) {
            Ok(next) => Ok(next),
            Err(e) => Err(anyhow!("time out of range: {e}")),
        }
    }
}
