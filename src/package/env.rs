//! Typed access to environment variables.
//!
//! Everything that reads the process environment goes through here, so the
//! rules about blank values, trimming and fallbacks live in one place instead
//! of being repeated in every loader.
//!
//! Unlike the Go helpers this was ported from, a value that is present but
//! unparseable is an error rather than a silent fallback. A typo in a
//! deployment should stop the process at start up, not quietly run with a
//! default nobody asked for.

use std::env;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("environment variable `{key}` is required")]
    Missing { key: &'static str },
    #[error("environment variable `{key}` has an invalid value: {value}")]
    Invalid { key: &'static str, value: String },
}

/// Reads a variable, treating "unset" and "set to whitespace" the same way.
pub fn string(key: &'static str) -> Option<String> {
    match env::var(key) {
        Ok(value) if !value.trim().is_empty() => Some(value.trim().to_owned()),
        _ => None,
    }
}

/// Reads a variable, falling back when it is unset or blank.
pub fn string_or(key: &'static str, fallback: &str) -> String {
    string(key).unwrap_or_else(|| fallback.to_owned())
}

/// Reads a variable that the application cannot start without.
pub fn required(key: &'static str) -> Result<String, Error> {
    string(key).ok_or(Error::Missing { key })
}

/// Reads and parses a variable into any type that knows how to parse itself.
pub fn parsed<T: FromStr>(key: &'static str) -> Result<Option<T>, Error> {
    match string(key) {
        None => Ok(None),
        Some(value) => value
            .parse::<T>()
            .map(Some)
            .map_err(|_| Error::Invalid { key, value }),
    }
}

/// Reads and parses a variable, falling back when it is unset or blank.
pub fn parsed_or<T: FromStr>(key: &'static str, fallback: T) -> Result<T, Error> {
    Ok(parsed(key)?.unwrap_or(fallback))
}

/// Reads and parses a variable, falling back to the type's own default.
pub fn parsed_or_default<T: FromStr + Default>(key: &'static str) -> Result<T, Error> {
    Ok(parsed(key)?.unwrap_or_default())
}

/// Reads a boolean, accepting the spellings people actually write in a `.env`
/// file rather than only the two `bool::from_str` knows about.
pub fn boolean(key: &'static str) -> Result<Option<bool>, Error> {
    let Some(value) = string(key) else {
        return Ok(None);
    };

    match value.to_ascii_lowercase().as_str() {
        "1" | "t" | "true" | "y" | "yes" | "on" => Ok(Some(true)),
        "0" | "f" | "false" | "n" | "no" | "off" => Ok(Some(false)),
        _ => Err(Error::Invalid { key, value }),
    }
}

/// Reads a boolean, falling back when it is unset or blank.
pub fn boolean_or(key: &'static str, fallback: bool) -> Result<bool, Error> {
    Ok(boolean(key)?.unwrap_or(fallback))
}

/// Reads a comma separated list, trimming each entry and dropping the empty
/// ones. A value that holds nothing but separators counts as unset, so
/// `FOO=" , "` falls back rather than producing an empty list.
pub fn list(key: &'static str) -> Option<Vec<String>> {
    let value = string(key)?;

    let entries: Vec<String> = value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    if entries.is_empty() {
        None
    } else {
        Some(entries)
    }
}

/// Reads a comma separated list, falling back when it is unset or blank.
pub fn list_or(key: &'static str, fallback: &[&str]) -> Vec<String> {
    list(key).unwrap_or_else(|| fallback.iter().map(|entry| (*entry).to_owned()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests in a binary share one process environment, so every test owns a
    /// key no other test touches.
    fn set(key: &str, value: &str) {
        env::set_var(key, value);
    }

    #[test]
    fn reads_a_string_and_trims_it() {
        set("ENV_TEST_STRING", "  hello  ");

        assert_eq!(string("ENV_TEST_STRING").as_deref(), Some("hello"));
    }

    #[test]
    fn a_blank_string_counts_as_unset() {
        set("ENV_TEST_BLANK", "   ");

        assert_eq!(string("ENV_TEST_BLANK"), None);
        assert_eq!(string_or("ENV_TEST_BLANK", "fallback"), "fallback");
    }

    #[test]
    fn an_unset_string_falls_back() {
        assert_eq!(string("ENV_TEST_ABSENT"), None);
        assert_eq!(string_or("ENV_TEST_ABSENT", "fallback"), "fallback");
    }

    #[test]
    fn a_required_variable_reports_the_key_it_is_missing() {
        assert_eq!(
            required("ENV_TEST_REQUIRED"),
            Err(Error::Missing {
                key: "ENV_TEST_REQUIRED"
            })
        );
    }

    #[test]
    fn parses_into_the_requested_type() {
        set("ENV_TEST_PORT", "8080");

        assert_eq!(parsed::<u16>("ENV_TEST_PORT").unwrap(), Some(8080));
        assert_eq!(parsed_or("ENV_TEST_PORT", 3000_u16).unwrap(), 8080);
        assert_eq!(parsed_or("ENV_TEST_NO_PORT", 3000_u16).unwrap(), 3000);
    }

    #[test]
    fn an_unparseable_value_is_an_error_not_a_fallback() {
        set("ENV_TEST_BAD_PORT", "eighty");

        assert_eq!(
            parsed::<u16>("ENV_TEST_BAD_PORT"),
            Err(Error::Invalid {
                key: "ENV_TEST_BAD_PORT",
                value: "eighty".to_owned()
            })
        );
        assert!(parsed_or("ENV_TEST_BAD_PORT", 3000_u16).is_err());
    }

    #[test]
    fn reads_the_boolean_spellings_people_actually_write() {
        for truthy in ["1", "t", "true", "TRUE", "y", "yes", "on"] {
            set("ENV_TEST_BOOLEAN", truthy);
            assert_eq!(boolean("ENV_TEST_BOOLEAN").unwrap(), Some(true), "{truthy}");
        }

        for falsy in ["0", "f", "false", "FALSE", "n", "no", "off"] {
            set("ENV_TEST_BOOLEAN", falsy);
            assert_eq!(boolean("ENV_TEST_BOOLEAN").unwrap(), Some(false), "{falsy}");
        }
    }

    #[test]
    fn an_unparseable_boolean_is_an_error() {
        set("ENV_TEST_BAD_BOOLEAN", "maybe");

        assert!(boolean("ENV_TEST_BAD_BOOLEAN").is_err());
        assert!(boolean_or("ENV_TEST_BAD_BOOLEAN", true).is_err());
        assert!(boolean_or("ENV_TEST_NO_BOOLEAN", true).unwrap());
    }

    #[test]
    fn splits_a_list_and_drops_the_empty_entries() {
        set("ENV_TEST_ORIGINS", " http://a.test , ,http://b.test ");

        assert_eq!(
            list("ENV_TEST_ORIGINS"),
            Some(vec!["http://a.test".to_owned(), "http://b.test".to_owned()])
        );
    }

    #[test]
    fn a_single_entry_is_still_a_list() {
        set("ENV_TEST_ONE_ORIGIN", "http://only.test");

        assert_eq!(
            list("ENV_TEST_ONE_ORIGIN"),
            Some(vec!["http://only.test".to_owned()])
        );
    }

    #[test]
    fn a_list_of_nothing_but_separators_falls_back() {
        set("ENV_TEST_EMPTY_LIST", "  ,  ");

        assert_eq!(list("ENV_TEST_EMPTY_LIST"), None);
        assert_eq!(list_or("ENV_TEST_EMPTY_LIST", &["*"]), vec!["*".to_owned()]);
    }
}
