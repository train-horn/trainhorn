use std::fmt;

/// Error returned when a calculator receives an invalid argument.
///
/// The message names the offending parameter, e.g.
/// `"distance must be greater than 0, got -5"`.
///
/// ```
/// let err = trainhorn::acoustics::spl_at_distance(150.0, -5.0, 10.0).unwrap_err();
/// assert_eq!(err.to_string(), "distance must be greater than 0, got -5");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputError {
    message: String,
}

impl InputError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        InputError {
            message: message.into(),
        }
    }

    /// The human-readable error message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for InputError {}

/// Return `value` if it is finite.
pub(crate) fn number(name: &str, value: f64) -> crate::Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(InputError::new(format!("{name} must be finite, got {value}")))
    }
}

/// Return `value` if it is finite and > 0.
pub(crate) fn positive(name: &str, value: f64) -> crate::Result<f64> {
    let v = number(name, value)?;
    if v <= 0.0 {
        Err(InputError::new(format!(
            "{name} must be greater than 0, got {v}"
        )))
    } else {
        Ok(v)
    }
}

/// Return `value` if it is finite and >= 0.
pub(crate) fn non_negative(name: &str, value: f64) -> crate::Result<f64> {
    let v = number(name, value)?;
    if v < 0.0 {
        Err(InputError::new(format!("{name} must be 0 or greater, got {v}")))
    } else {
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validators() {
        assert_eq!(number("x", 1.5), Ok(1.5));
        assert!(number("x", f64::NAN).is_err());
        assert!(number("x", f64::INFINITY).is_err());
        assert!(positive("x", 0.0).is_err());
        assert_eq!(non_negative("x", 0.0), Ok(0.0));
        assert!(non_negative("x", -0.1).is_err());
        assert_eq!(
            positive("distance", -5.0).unwrap_err().message(),
            "distance must be greater than 0, got -5"
        );
    }

    #[test]
    fn is_std_error() {
        fn takes(_: &dyn std::error::Error) {}
        takes(&InputError::new("x"));
    }
}
