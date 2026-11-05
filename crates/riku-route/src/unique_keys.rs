//! Refuses JSON that repeats a key in any object.
//!
//! serde keeps the last of two equal keys, so a request such as
//! `{"app":"a","app":"b"}` would be read differently by a parser that keeps
//! the first. The helper accepts no ambiguity, so the check runs before the
//! request is read into types.

use std::collections::HashSet;
use std::fmt;

use anyhow::{anyhow, Result};
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

struct Unique;

struct UniqueVisitor;

impl<'de> Visitor<'de> for UniqueVisitor {
    type Value = Unique;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<Unique, E> {
        Ok(Unique)
    }

    fn visit_i64<E: de::Error>(self, _: i64) -> Result<Unique, E> {
        Ok(Unique)
    }

    fn visit_u64<E: de::Error>(self, _: u64) -> Result<Unique, E> {
        Ok(Unique)
    }

    fn visit_f64<E: de::Error>(self, _: f64) -> Result<Unique, E> {
        Ok(Unique)
    }

    fn visit_str<E: de::Error>(self, _: &str) -> Result<Unique, E> {
        Ok(Unique)
    }

    fn visit_unit<E: de::Error>(self) -> Result<Unique, E> {
        Ok(Unique)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Unique, A::Error> {
        loop {
            match seq.next_element::<Unique>() {
                Ok(Some(_)) => {}
                Ok(None) => return Ok(Unique),
                Err(e) => return Err(e),
            }
        }
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Unique, A::Error> {
        let mut seen = HashSet::new();
        loop {
            let key = match map.next_key::<String>() {
                Ok(Some(key)) => key,
                Ok(None) => return Ok(Unique),
                Err(e) => return Err(e),
            };
            if !seen.insert(key.clone()) {
                return Err(de::Error::custom(format!("duplicate key {key:?}")));
            }
            if let Err(e) = map.next_value::<Unique>() {
                return Err(de::Error::custom(format!("in the value of {key:?}: {e}")));
            }
        }
    }
}

impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(UniqueVisitor)
    }
}

/// Fail when any object in `bytes` has the same key twice.
pub fn refuse_duplicate_keys(bytes: &[u8]) -> Result<()> {
    match serde_json::from_slice::<Unique>(bytes) {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow!("{e}")),
    }
}
