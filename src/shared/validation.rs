use validator::{ValidationError as FieldError, ValidationErrors, ValidationErrorsKind};

use crate::shared::errdef::Error;

/// `strong-pwd` in the Go validator: at least 8 characters with an uppercase,
/// a lowercase, a digit and a symbol.
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

/// Same rules as [`strong_password`], but returning the specific reason so the
/// user service can report it on a partial update.
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

/// Turns `validator` output into the flat `field -> [message]` map the API
/// contract exposes, including nested and indexed paths (`roles.0.name`).
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
}
