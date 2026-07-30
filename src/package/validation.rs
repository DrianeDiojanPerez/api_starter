#[cfg(test)]
use validator::Validate;
use validator::{ValidationError as FieldError, ValidationErrors, ValidationErrorsKind};

use crate::package::errdef::Error;

pub fn strong_password(password: &str) -> Result<(), FieldError> {
    let has_min_len = password.chars().count() >= 8;
    let has_upper = password.chars().any(char::is_uppercase);
    let has_lower = password.chars().any(char::is_lowercase);
    let has_number = password.chars().any(|c| c.is_ascii_digit());
    let has_special = password
        .chars()
        .any(|c| !c.is_alphanumeric() && !c.is_whitespace());

    if has_min_len && has_upper && has_lower && has_number && has_special {
        return Ok(());
    }

    Err(FieldError::new("strong-pwd"))
}

pub fn password_checker(password: &str) -> Result<(), String> {
    if password.chars().count() < 8 {
        return Err("password is too short. It should be at least 8 characters long".to_owned());
    }
    if !password.chars().any(char::is_uppercase) {
        return Err("password should contain at least one uppercase letter".to_owned());
    }
    if !password.chars().any(char::is_lowercase) {
        return Err("password should contain at least one lowercase letter".to_owned());
    }
    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err("password should contain at least one digit".to_owned());
    }
    if !password
        .chars()
        .any(|c| !c.is_alphanumeric() && !c.is_whitespace())
    {
        return Err("password should contain at least one special character".to_owned());
    }
    Ok(())
}

pub fn to_error(errors: ValidationErrors) -> Error {
    let mut error = Error::validation("failed payload validation");
    collect(&errors, "", &mut error);
    error
}

fn collect(errors: &ValidationErrors, prefix: &str, target: &mut Error) {
    for (field, kind) in errors.errors() {
        let path = if prefix.is_empty() {
            field.to_string()
        } else {
            format!("{prefix}.{field}")
        };

        match kind {
            ValidationErrorsKind::Field(field_errors) => {
                for field_error in field_errors {
                    target.push_violation(path.clone(), message_for(field_error));
                }
            }
            ValidationErrorsKind::Struct(nested) => collect(nested, &path, target),
            ValidationErrorsKind::List(items) => {
                for (index, nested) in items {
                    collect(nested, &format!("{path}.{index}"), target);
                }
            }
        }
    }
}

fn message_for(error: &FieldError) -> String {
    let param = |key: &str| {
        error
            .params
            .get(key)
            .map(|value| value.to_string().trim_matches('"').to_owned())
            .unwrap_or_default()
    };

    match error.code.as_ref() {
        "required" => "field is required and cannot be empty".to_owned(),
        "email" => "field must be of email format".to_owned(),
        "length" => format!("field must be a minimum length of {}", param("min")),
        "url" => "filed must be of URI format".to_owned(),
        "range" => "field is outside of the accepted range".to_owned(),
        "strong-pwd" => {
            "field requires at least 8 characters with an uppercase letter, a symbol and a numeric value."
                .to_owned()
        }
        _ => "invalid value for field".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_strong_password() {
        assert!(strong_password("Sup3r$ecret").is_ok());
    }

    #[test]
    fn rejects_a_password_without_a_symbol() {
        assert!(strong_password("Sup3rsecret").is_err());
        assert_eq!(
            password_checker("Sup3rsecret"),
            Err("password should contain at least one special character".to_owned())
        );
    }

    #[test]
    fn reports_each_missing_password_rule_separately() {
        assert_eq!(
            password_checker("Ab1$"),
            Err("password is too short. It should be at least 8 characters long".to_owned())
        );
        assert_eq!(
            password_checker("sup3r$ecret"),
            Err("password should contain at least one uppercase letter".to_owned())
        );
        assert_eq!(
            password_checker("SUP3R$ECRET"),
            Err("password should contain at least one lowercase letter".to_owned())
        );
        assert_eq!(
            password_checker("Super$ecret"),
            Err("password should contain at least one digit".to_owned())
        );
        assert!(password_checker("Sup3r$ecret").is_ok());
    }

    #[derive(Debug, Validate)]
    struct Inner {
        #[validate(email)]
        email: String,
    }

    #[derive(Debug, Validate)]
    struct Outer {
        #[validate(length(min = 2))]
        name: String,
        #[validate(nested)]
        inner: Inner,
        #[validate(nested)]
        items: Vec<Inner>,
    }

    fn violations(error: &Error) -> &std::collections::BTreeMap<String, Vec<String>> {
        match error {
            Error::Validation(err) => &err.field_violations,
            other => panic!("expected a validation error, got {other:?}"),
        }
    }

    #[test]
    fn flattens_nested_and_indexed_field_paths() {
        let payload = Outer {
            name: "a".to_owned(),
            inner: Inner {
                email: "not-an-email".to_owned(),
            },
            items: vec![
                Inner {
                    email: "ok@example.com".to_owned(),
                },
                Inner {
                    email: "also-not-an-email".to_owned(),
                },
            ],
        };

        let error = to_error(payload.validate().expect_err("validation should fail"));
        let violations = violations(&error);

        assert!(violations.contains_key("name"));
        assert_eq!(
            violations.get("inner.email").map(Vec::as_slice),
            Some(["field must be of email format".to_owned()].as_slice())
        );
        // The failing element keeps its index, the passing one is absent.
        assert!(violations.contains_key("items.1.email"));
        assert!(!violations.contains_key("items.0.email"));
    }

    #[test]
    fn translates_the_builtin_rules_into_readable_messages() {
        #[derive(Validate)]
        struct Payload {
            #[validate(length(min = 8))]
            token: String,
            #[validate(email)]
            email: String,
            #[validate(url)]
            callback_uri: String,
            #[validate(custom(function = "strong_password"))]
            password: String,
        }

        let error = to_error(
            Payload {
                token: "short".to_owned(),
                email: "nope".to_owned(),
                callback_uri: "nope".to_owned(),
                password: "weak".to_owned(),
            }
            .validate()
            .expect_err("validation should fail"),
        );
        let violations = violations(&error);

        assert_eq!(
            violations["token"][0],
            "field must be a minimum length of 8"
        );
        assert_eq!(violations["email"][0], "field must be of email format");
        assert_eq!(violations["callback_uri"][0], "filed must be of URI format");
        assert!(violations["password"][0].contains("at least 8 characters"));
    }
}
