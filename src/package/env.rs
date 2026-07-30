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

pub fn string(key: &'static str) -> Option<String> {
    match env::var(key) {
        Ok(value) if !value.trim().is_empty() => Some(value.trim().to_owned()),
        _ => None,
    }
}

pub fn string_or(key: &'static str, fallback: &str) -> String {
    string(key).unwrap_or_else(|| fallback.to_owned())
}

pub fn required(key: &'static str) -> Result<String, Error> {
    string(key).ok_or(Error::Missing { key })
}

pub fn u16(key: &'static str) -> Result<Option<u16>, Error> {
    parsed(key)
}

pub fn u16_or(key: &'static str, fallback: u16) -> Result<u16, Error> {
    Ok(parsed(key)?.unwrap_or(fallback))
}

pub fn u32(key: &'static str) -> Result<Option<u32>, Error> {
    parsed(key)
}

pub fn u32_or(key: &'static str, fallback: u32) -> Result<u32, Error> {
    Ok(parsed(key)?.unwrap_or(fallback))
}

pub fn i64(key: &'static str) -> Result<Option<i64>, Error> {
    parsed(key)
}

pub fn i64_or(key: &'static str, fallback: i64) -> Result<i64, Error> {
    Ok(parsed(key)?.unwrap_or(fallback))
}

/// The one generic reader: a package that must not know about `config` cannot
/// name the enums that live there.
pub fn variant_or_default<T: FromStr + Default>(key: &'static str) -> Result<T, Error> {
    Ok(parsed(key)?.unwrap_or_default())
}

/// Private on purpose, so callers ask for a concrete type by name instead of
/// reaching for a generic escape hatch.
fn parsed<T: FromStr>(key: &'static str) -> Result<Option<T>, Error> {
    match string(key) {
        None => Ok(None),
        Some(value) => value
            .parse::<T>()
            .map(Some)
            .map_err(|_| Error::Invalid { key, value }),
    }
}

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

pub fn boolean_or(key: &'static str, fallback: bool) -> Result<bool, Error> {
    Ok(boolean(key)?.unwrap_or(fallback))
}

pub fn vec(key: &'static str) -> Option<Vec<String>> {
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

pub fn vec_or(key: &'static str, fallback: &[&str]) -> Vec<String> {
    vec(key).unwrap_or_else(|| fallback.iter().map(|entry| (*entry).to_owned()).collect())
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
    fn reads_each_number_at_the_width_it_was_asked_for() {
        set("ENV_TEST_PORT", "8080");
        set("ENV_TEST_POOL", "25");
        set("ENV_TEST_TTL", "-604800");

        assert_eq!(u16("ENV_TEST_PORT").unwrap(), Some(8080));
        assert_eq!(u16_or("ENV_TEST_PORT", 3000).unwrap(), 8080);
        assert_eq!(u16_or("ENV_TEST_NO_PORT", 3000).unwrap(), 3000);

        assert_eq!(u32("ENV_TEST_POOL").unwrap(), Some(25));
        assert_eq!(u32_or("ENV_TEST_NO_POOL", 10).unwrap(), 10);

        assert_eq!(i64("ENV_TEST_TTL").unwrap(), Some(-604_800));
        assert_eq!(i64_or("ENV_TEST_NO_TTL", 3600).unwrap(), 3600);
    }

    #[test]
    fn a_number_that_does_not_fit_the_width_is_rejected() {
        set("ENV_TEST_WIDE_PORT", "70000");
        set("ENV_TEST_NEGATIVE_POOL", "-1");

        assert!(u16("ENV_TEST_WIDE_PORT").is_err(), "70000 overflows a u16");
        assert!(u32("ENV_TEST_NEGATIVE_POOL").is_err(), "a pool is unsigned");
        assert_eq!(i64("ENV_TEST_NEGATIVE_POOL").unwrap(), Some(-1));
    }

    #[test]
    fn an_unparseable_value_is_an_error_not_a_fallback() {
        set("ENV_TEST_BAD_PORT", "eighty");

        assert_eq!(
            u16("ENV_TEST_BAD_PORT"),
            Err(Error::Invalid {
                key: "ENV_TEST_BAD_PORT",
                value: "eighty".to_owned()
            })
        );
        assert!(u16_or("ENV_TEST_BAD_PORT", 3000).is_err());
    }

    #[test]
    fn a_named_variant_falls_back_to_its_own_default() {
        #[derive(Debug, Default, PartialEq)]
        enum Level {
            #[default]
            Info,
            Error,
        }

        impl FromStr for Level {
            type Err = ();

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value.to_ascii_lowercase().as_str() {
                    "info" => Ok(Self::Info),
                    "error" => Ok(Self::Error),
                    _ => Err(()),
                }
            }
        }

        set("ENV_TEST_LEVEL", "ERROR");

        assert_eq!(
            variant_or_default::<Level>("ENV_TEST_LEVEL").unwrap(),
            Level::Error
        );
        assert_eq!(
            variant_or_default::<Level>("ENV_TEST_NO_LEVEL").unwrap(),
            Level::Info
        );

        set("ENV_TEST_BAD_LEVEL", "chatty");
        assert!(variant_or_default::<Level>("ENV_TEST_BAD_LEVEL").is_err());
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
            vec("ENV_TEST_ORIGINS"),
            Some(vec!["http://a.test".to_owned(), "http://b.test".to_owned()])
        );
    }

    #[test]
    fn a_single_entry_is_still_a_list() {
        set("ENV_TEST_ONE_ORIGIN", "http://only.test");

        assert_eq!(
            vec("ENV_TEST_ONE_ORIGIN"),
            Some(vec!["http://only.test".to_owned()])
        );
    }

    #[test]
    fn a_list_of_nothing_but_separators_falls_back() {
        set("ENV_TEST_EMPTY_VEC", "  ,  ");

        assert_eq!(vec("ENV_TEST_EMPTY_VEC"), None);
        assert_eq!(vec_or("ENV_TEST_EMPTY_VEC", &["*"]), vec!["*".to_owned()]);
    }
}
