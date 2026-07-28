use std::fmt;
use std::str::FromStr;

/// Verbosity, lowest to highest. Parsed from `LOGGER_LEVEL`, which takes a
/// level name and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }

    /// Every accepted value, for the error message when parsing fails.
    pub const VARIANTS: [&'static str; 5] = ["trace", "debug", "info", "warn", "error"];
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Only the names parse. A stray number or typo is rejected at start up rather
/// than silently logging at the wrong level.
impl FromStr for LogLevel {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "trace" => Ok(LogLevel::Trace),
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" | "warning" => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Logger {
    pub level: LogLevel,
    pub directory: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_level_name() {
        assert_eq!("trace".parse(), Ok(LogLevel::Trace));
        assert_eq!("debug".parse(), Ok(LogLevel::Debug));
        assert_eq!("info".parse(), Ok(LogLevel::Info));
        assert_eq!("warn".parse(), Ok(LogLevel::Warn));
        assert_eq!("error".parse(), Ok(LogLevel::Error));
    }

    #[test]
    fn is_case_and_whitespace_insensitive() {
        assert_eq!("  DEBUG  ".parse(), Ok(LogLevel::Debug));
        assert_eq!("Warn".parse(), Ok(LogLevel::Warn));
        assert_eq!("WARNING".parse(), Ok(LogLevel::Warn));
    }

    #[test]
    fn rejects_the_numeric_levels_the_go_service_used() {
        for numeric in ["-8", "-4", "0", "4", "8"] {
            assert_eq!(
                numeric.parse::<LogLevel>(),
                Err(()),
                "{numeric} should not parse"
            );
        }
    }

    #[test]
    fn rejects_anything_that_is_not_a_level() {
        assert_eq!("verbose".parse::<LogLevel>(), Err(()));
        assert_eq!("".parse::<LogLevel>(), Err(()));
    }

    #[test]
    fn renders_back_to_the_name_tracing_expects() {
        for name in LogLevel::VARIANTS {
            let level: LogLevel = name.parse().expect("the variant list should parse");

            assert_eq!(level.to_string(), name);
        }
    }

    #[test]
    fn defaults_to_info() {
        assert_eq!(LogLevel::default(), LogLevel::Info);
    }
}
