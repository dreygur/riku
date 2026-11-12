//! Walking a parsed TOML table into [`AppSettings`], one section at a time.

use std::collections::BTreeMap;

use super::keys::{self, Key, Scope};
use super::values::{self, Value};
use super::{processes, AppSettings, Problem, Source};

fn table_of<'a>(section: &str, value: &'a toml::Value) -> Result<&'a toml::Table, Problem> {
    match value.as_table() {
        Some(table) => Ok(table),
        None => Err(Problem {
            section: section.to_string(),
            key: None,
            message: format!("{section} must be a table"),
        }),
    }
}

fn checked(
    section: &str,
    name: &str,
    value: &toml::Value,
    source: Source,
) -> Result<(&'static Key, Value), Problem> {
    let path = format!("{section}.{name}");
    let key = match keys::find(&path) {
        Some(key) => key,
        None => return Err(Problem::at(section, name, format!("unknown key {path}"))),
    };
    if source == Source::Repo && key.scope == Scope::ServerOnly {
        return Err(Problem::at(
            section,
            name,
            format!("{path} is a server setting; set it with riku settings set"),
        ));
    }
    match values::check(key, value, source) {
        Ok(value) => Ok((key, value)),
        Err(message) => Err(Problem::at(section, name, message)),
    }
}

impl AppSettings {
    pub(super) fn check_runtime_version(&self) -> Result<(), Problem> {
        if self.value_of("runtime.version").is_some() && self.runtime_name().is_none() {
            return Err(Problem::at(
                "runtime",
                "version",
                "runtime.version needs runtime.name in the same file".to_string(),
            ));
        }
        Ok(())
    }

    fn add_section(
        &mut self,
        section: &str,
        value: &toml::Value,
        source: Source,
    ) -> Result<(), Problem> {
        let table = match table_of(section, value) {
            Ok(table) => table,
            Err(problem) => return Err(problem),
        };
        if section == "processes" || section == "cron" {
            if source == Source::Server {
                return Err(Problem {
                    section: section.to_string(),
                    key: None,
                    message: format!("[{section}] belongs in the app repo riku.toml"),
                });
            }
            let out = self.processes.get_or_insert_with(BTreeMap::new);
            return processes::add_section(section, table, out);
        }
        for (name, value) in table {
            match checked(section, name, value, source) {
                Ok((key, value)) => self.put(key, value),
                Err(problem) => return Err(problem),
            }
        }
        Ok(())
    }

    pub(super) fn from_table(root: &toml::Table, source: Source) -> Result<AppSettings, Problem> {
        let mut settings = AppSettings::default();
        for (section, value) in root {
            if let Err(problem) = settings.add_section(section, value, source) {
                return Err(problem);
            }
        }
        if let Err(problem) = settings.check_runtime_version() {
            return Err(problem);
        }
        match &settings.processes {
            Some(processes) => match processes::check_web_and_wsgi(processes) {
                Ok(()) => Ok(settings),
                Err(problem) => Err(problem),
            },
            None => Ok(settings),
        }
    }
}
