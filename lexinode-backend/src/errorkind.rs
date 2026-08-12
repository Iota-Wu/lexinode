use lexinode_core::error::Result as CoreResult;
use std::borrow::Cow;

/// Type alias for `Result<T, ErrorContext<Error>>`.
///
/// Used throughout the core crate to return domain-specific errors
/// wrapped with rich context information.
pub type Result<T> = CoreResult<T, BackendError>;

/// Convenience alias for `Cow<'static, str>`.
///
/// Allows accepting both zero-cost static string literals (`&'static str`)
/// and dynamically allocated strings (`String`).
type CowStr = Cow<'static, str>;

/// Core domain errors representing all expected failure modes within the application.
#[derive(thiserror::Error, Debug)]
pub enum BackendError {
    /// Fallback for database errors that don't match any more specific classification
    /// above (e.g. unique/check/foreign-key violations, permission errors). Most
    /// `sqlx::Error::Database` cases are mapped to a more specific variant instead.
    #[error("Database constraint or execution error: {0}")]
    Database(sqlx::Error),

    /// Returned when a requested entity/resource cannot be found in the database or cache.
    #[error("Entity not found: {resource} with ID '{id}'")]
    NotFound {
        /// The name or type of the resource (e.g., "user", "order").
        resource: CowStr,
        /// The unique identifier of the missing resource.
        id: CowStr,
    },

    /// Returned when user input or request payload fails domain business logic validation.
    #[error("Validation failed for field '{field}': {message}")]
    Validation {
        /// The payload or domain field that failed validation.
        field: CowStr,
        /// Description of why the validation failed.
        message: CowStr,
    },

    /// Returned when an operation conflicts with the current state (e.g., unique key violation).
    #[error("Conflict: {0}")]
    Conflict(CowStr),

    /// Returned when an authenticated user lacks permissions for the requested operation.
    #[error("Permission denied: {0}")]
    Forbidden(CowStr),

    /// Returned when authentication credentials are missing, invalid, or expired.
    #[error("Unauthenticated: {0}")]
    Unauthorized(CowStr),

    /// Returned for unexpected internal software defects or invariant violations.
    /// Captures the source code location (`file:line:column`) where the error was instantiated.
    #[error("Internal error at {location}: {message}")]
    Internal {
        /// Explanation of the internal error or invariant failure.
        message: CowStr,
        /// Source code location where this error was created.
        location: &'static std::panic::Location<'static>,
    },
}

impl BackendError {
    /// Creates an [`BackendError::Internal`] variant, automatically capturing the caller's
    /// source code position (`file:line:column`) via `#[track_caller]`.
    ///
    /// # Arguments
    ///
    /// * `msg` - Contextual message describing the internal issue.
    #[track_caller]
    pub fn internal(msg: impl Into<CowStr>) -> Self {
        Self::Internal {
            message: msg.into(),
            location: std::panic::Location::caller(),
        }
    }
}

impl From<sqlx::Error> for BackendError {
    /// Maps low-level [`sqlx::Error`] variants into higher-level domain errors where appropriate.
    ///
    /// - Converts [`sqlx::Error::RowNotFound`] to [`BackendError::NotFound`].
    /// - Converts database unique constraint violations to [`BackendError::Conflict`].
    /// - Wraps all other database execution errors inside [`BackendError::Database`].
    fn from(err: sqlx::Error) -> Self {
        match err {
            // Map unique violation to Conflict
            sqlx::Error::Database(db_err) => {
                if db_err.is_unique_violation() {
                    return Self::Conflict(db_err.message().to_owned().into());
                } else if db_err.is_check_violation() {
                    return Self::Validation {
                        field: db_err
                            .constraint()
                            .unwrap_or("check_constraint")
                            .to_owned()
                            .into(),
                        message: db_err.message().to_owned().into(),
                    };
                } else if db_err.is_foreign_key_violation() {
                    return Self::Validation {
                        field: db_err
                            .constraint()
                            .unwrap_or("foreign_key")
                            .to_owned()
                            .into(),
                        message: "Referenced entity does not exist".into(),
                    };
                } else if db_err.code().as_deref() == Some("42501") {
                    return Self::Forbidden(db_err.to_string().into());
                }
                Self::Database(sqlx::Error::Database(db_err))
            }

            // Map pool timeout or worker crash to internal error
            sqlx::Error::PoolTimedOut => Self::internal(
                "Database connection pool exhausted. Consider increasing MAX_CONNECTIONS in your config.",
            ),
            // Map SQLx RowNotFound directly to domain NotFound
            sqlx::Error::RowNotFound => Self::NotFound {
                resource: "row".into(),
                id: "unknown".into(),
            },
            // Map type not found to internal error
            sqlx::Error::TypeNotFound { type_name } => {
                if type_name.eq_ignore_ascii_case("vector") {
                    Self::internal(
                        "Database type 'vector' not found. Please ensure the pgvector extension is installed in PostgreSQL (`CREATE EXTENSION IF NOT EXISTS vector;`).",
                    )
                } else {
                    Self::internal(format!(
                        "Database custom type '{type_name}' not found in schema."
                    ))
                }
            }
            // Map other database errors to generic Database
            other => Self::internal(format!("Database layer error: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lexinode_core::error::ErrorContext;

    // =====================================================================
    // 1. Error Variants & Helper Method Tests
    // =====================================================================

    #[test]
    fn test_internal_error_captures_caller_location() {
        use std::path::Path;
        // Line number check: Error::internal is called right on this line
        let current_line = line!() + 1;
        let err = BackendError::internal("unexpected null pointer");

        if let BackendError::Internal { message, location } = err {
            assert_eq!(message, "unexpected null pointer");
            assert_eq!(location.line(), current_line);

            let Some(extension) = Path::new(location.file()).extension() else {
                panic!("Expected file extension");
            };
            assert!(
                extension == "rs",
                "Expected file extension to be 'rs', got '{extension:?}'"
            );
        } else {
            panic!("Expected BackendError::Internal variant");
        }
    }

    #[test]
    fn test_error_display_formatting() {
        let not_found = BackendError::NotFound {
            resource: "User".into(),
            id: "usr_123".into(),
        };
        assert_eq!(
            not_found.to_string(),
            "Entity not found: User with ID 'usr_123'"
        );

        let validation = BackendError::Validation {
            field: "email".into(),
            message: "invalid format".into(),
        };
        assert_eq!(
            validation.to_string(),
            "Validation failed for field 'email': invalid format"
        );
    }

    // =====================================================================
    // 2. sqlx::Error Mapping Tests (From<sqlx::Error>)
    // =====================================================================

    #[test]
    fn test_sqlx_row_not_found_mapping() {
        let sqlx_err = sqlx::Error::RowNotFound;
        let domain_err = sqlx_err.into();

        match domain_err {
            BackendError::NotFound { resource, id } => {
                assert_eq!(resource, "row");
                assert_eq!(id, "unknown");
            }
            _ => panic!("Expected BackendError::NotFound, got {domain_err:?}"),
        }
    }

    #[test]
    fn test_sqlx_generic_error_mapping() {
        let sqlx_err = sqlx::Error::PoolTimedOut;
        let domain_err = sqlx_err.into();

        match domain_err {
            BackendError::Internal { message, .. } => {
                assert_eq!(
                    message,
                    "Database connection pool exhausted. Consider increasing MAX_CONNECTIONS in your config."
                );
            }
            _ => panic!("Expected BackendError::Internal, got {domain_err:?}"),
        }
    }

    // =====================================================================
    // 3. Size of Error and ErrorContext
    // =====================================================================

    #[test]
    fn test_sizes() {
        use std::mem::size_of;

        assert_eq!(
            size_of::<Box<ErrorContext<BackendError>>>(),
            size_of::<usize>(),
            "`Box<ErrorContext<BackendError>>` should always be the size of a pointer"
        );

        println!("ErrorContext: {}", size_of::<ErrorContext<BackendError>>()); // Last test is 104 bytes
        println!("Error: {}", size_of::<BackendError>()); // Last test is 56 bytes
    }
}
