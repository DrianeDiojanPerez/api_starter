use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct Server {
    pub port: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Environment {
    Local,
    #[default]
    Development,
    Production,
}

impl Environment {
    pub const fn as_str(self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Development => "development",
            Environment::Production => "production",
        }
    }

    pub const VARIANTS: [&'static str; 3] = ["local", "development", "production"];

    pub const fn is_production(self) -> bool {
        matches!(self, Environment::Production)
    }
}

impl fmt::Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Environment {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "local" => Ok(Environment::Local),
            "development" => Ok(Environment::Development),
            "production" => Ok(Environment::Production),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Deployment {
    pub name: String,
    pub environment: Environment,
    pub time_zone: String,
}

impl Deployment {
    pub fn is_production(&self) -> bool {
        self.environment.is_production()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_environment_name() {
        assert_eq!("local".parse(), Ok(Environment::Local));
        assert_eq!("development".parse(), Ok(Environment::Development));
        assert_eq!("production".parse(), Ok(Environment::Production));
    }

    #[test]
    fn is_case_and_whitespace_insensitive() {
        assert_eq!("  DEVELOPMENT  ".parse(), Ok(Environment::Development));
        assert_eq!("Production".parse(), Ok(Environment::Production));
    }

    #[test]
    fn rejects_anything_outside_the_three_names() {
        for value in ["prod", "dev", "uat", "staging", "test", ""] {
            assert_eq!(
                value.parse::<Environment>(),
                Err(()),
                "{value} should not parse"
            );
        }
    }

    #[test]
    fn only_production_counts_as_production() {
        assert!(Environment::Production.is_production());
        assert!(!Environment::Development.is_production());
        assert!(!Environment::Local.is_production());
    }

    #[test]
    fn renders_back_to_the_name_it_parsed_from() {
        for name in Environment::VARIANTS {
            let environment: Environment = name.parse().expect("the variant list should parse");

            assert_eq!(environment.to_string(), name);
        }
    }

    #[test]
    fn defaults_to_development() {
        assert_eq!(Environment::default(), Environment::Development);
    }
}
