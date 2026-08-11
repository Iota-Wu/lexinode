use std::borrow::Cow;

/// Type alias for `Result<T, ErrorContext>`.
///
/// Used throughout the core crate to return domain-specific errors
/// wrapped with rich context information.
pub type Result<T> = std::result::Result<T, Box<ErrorContext>>;

/// Convenience alias for `Cow<'static, str>`.
///
/// Allows accepting both zero-cost static string literals (`&'static str`)
/// and dynamically allocated strings (`String`).
type CowStr = Cow<'static, str>;

/// Core domain errors representing all expected failure modes within the application.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Wrapper for underlying PostgreSQL/SQLx database execution errors.
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

impl Error {
    /// Creates an [`Error::Internal`] variant, automatically capturing the caller's
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

impl From<sqlx::Error> for Error {
    /// Maps low-level [`sqlx::Error`] variants into higher-level domain errors where appropriate.
    ///
    /// - Converts [`sqlx::Error::RowNotFound`] to [`Error::NotFound`].
    /// - Converts database unique constraint violations to [`Error::Conflict`].
    /// - Wraps all other database execution errors inside [`Error::Database`].
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

/// Rich operational wrapper that attaches context to an underlying domain [`Error`].
///
/// Designed to provide meaningful error messages by pairing what action was being
/// performed (`op`) with the root error (`source`) and optional debug context (`details`).
#[derive(Debug)]
pub struct ErrorContext {
    /// High-level description of the operation being executed (e.g., "fetch user profile").
    pub op: Option<CowStr>,
    /// The underlying domain error trigger.
    pub source: Error,
    /// Optional structured/unstructured metadata providing deeper diagnostic context.
    pub details: Option<CowStr>,
}

impl std::fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.op {
            Some(op) => write!(f, "Failed to {op}: {}", self.source)?,
            None => write!(f, "{}", self.source)?,
        }

        if let Some(ref details) = self.details {
            write!(f, " (details: {details})")?;
        }
        Ok(())
    }
}

impl std::error::Error for ErrorContext {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

impl<E> From<E> for Box<ErrorContext>
where
    E: Into<Error>,
{
    fn from(err: E) -> Self {
        Box::new(ErrorContext {
            op: None,
            source: err.into(),
            details: None,
        })
    }
}

/// Extension trait for [`std::result::Result`] providing fluent contextual error binding.
///
/// Allows chaining context onto any `Result<T, E>` where `E` can be converted into [`Error`].
pub trait ResultExt<T> {
    /// Wraps an error with an operation name.
    ///
    /// # Arguments
    ///
    /// * `op` - High-level description of the operation being attempted.
    fn context(self, op: impl Into<CowStr>) -> Result<T>;

    /// Wraps an error with an operation name and lazily computed extra details.
    ///
    /// # Arguments
    ///
    /// * `op` - High-level description of the operation being attempted.
    /// * `details_fn` - Closure returning extra contextual information, evaluated only on failure.
    fn with_context<S: Into<CowStr>>(
        self,
        op: impl Into<CowStr>,
        details_fn: impl FnOnce() -> S,
    ) -> Result<T>;
}

impl<T, E> ResultExt<T> for std::result::Result<T, E>
where
    E: Into<Error>,
{
    fn context(self, op: impl Into<CowStr>) -> Result<T> {
        self.map_err(|err| {
            Box::new(ErrorContext {
                op: Some(op.into()),
                source: err.into(),
                details: None,
            })
        })
    }

    fn with_context<S: Into<CowStr>>(
        self,
        op: impl Into<CowStr>,
        details_fn: impl FnOnce() -> S,
    ) -> Result<T> {
        self.map_err(|err| {
            Box::new(ErrorContext {
                op: Some(op.into()),
                source: err.into(),
                details: Some(details_fn().into()),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use std::assert_matches;

    // =====================================================================
    // 1. Error Variants & Helper Method Tests
    // =====================================================================

    #[test]
    fn test_internal_error_captures_caller_location() {
        use std::path::Path;
        // Line number check: Error::internal is called right on this line
        let current_line = line!() + 1;
        let err = Error::internal("unexpected null pointer");

        if let Error::Internal { message, location } = err {
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
            panic!("Expected Error::Internal variant");
        }
    }

    #[test]
    fn test_error_display_formatting() {
        let not_found = Error::NotFound {
            resource: "User".into(),
            id: "usr_123".into(),
        };
        assert_eq!(
            not_found.to_string(),
            "Entity not found: User with ID 'usr_123'"
        );

        let validation = Error::Validation {
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
            Error::NotFound { resource, id } => {
                assert_eq!(resource, "row");
                assert_eq!(id, "unknown");
            }
            _ => panic!("Expected Error::NotFound, got {domain_err:?}"),
        }
    }

    #[test]
    fn test_sqlx_generic_error_mapping() {
        let sqlx_err = sqlx::Error::PoolTimedOut;
        let domain_err = sqlx_err.into();

        match domain_err {
            Error::Internal { message, .. } => {
                assert_eq!(
                    message,
                    "Database connection pool exhausted. Consider increasing MAX_CONNECTIONS in your config."
                );
            }
            _ => panic!("Expected Error::Internal, got {domain_err:?}"),
        }
    }

    // =====================================================================
    // 3. ErrorContext & Display Tests
    // =====================================================================

    #[test]
    fn test_error_context_display_without_details() {
        let ctx = ErrorContext {
            op: Some("fetch user profile".into()),
            source: Error::NotFound {
                resource: "User".into(),
                id: "42".into(),
            },
            details: None,
        };

        assert_eq!(
            ctx.to_string(),
            "Failed to fetch user profile: Entity not found: User with ID '42'"
        );
    }

    #[test]
    fn test_error_context_display_with_details() {
        let ctx = ErrorContext {
            op: Some("process payment".into()),
            source: Error::Conflict("insufficient funds".into()),
            details: Some("account balance: $0.00".into()),
        };

        assert_eq!(
            ctx.to_string(),
            "Failed to process payment: Conflict: insufficient funds (details: account balance: $0.00)"
        );
    }

    #[test]
    fn test_std_error_source_chain() {
        use std::error::Error as StdError;

        let ctx = ErrorContext {
            op: Some("delete database".into()),
            source: Error::Forbidden("admin rights required".into()),
            details: None,
        };

        // Verify std::error::Error::source properly returns the underlying domain error
        let source = ctx.source().expect("should have source error");
        assert_eq!(
            source.to_string(),
            "Permission denied: admin rights required"
        );
    }

    // =====================================================================
    // 4. ResultExt Extension Trait Tests
    // =====================================================================

    #[allow(clippy::unnecessary_wraps)]
    fn mock_op_success() -> std::result::Result<&'static str, Error> {
        Ok("success")
    }

    fn mock_op_failure() -> std::result::Result<&'static str, Error> {
        Err(Error::Unauthorized("missing header".into()))
    }

    fn mock_sqlx_failure() -> std::result::Result<&'static str, sqlx::Error> {
        Err(sqlx::Error::RowNotFound)
    }

    fn mock_direct_question_mark() -> Result<&'static str> {
        let _ = mock_sqlx_failure()?; // Check that direct question mark work to convert sqlx error to Box<ErrorContext> without `op`
        Ok("unreachable")
    }

    #[test]
    fn test_result_ext_context_on_ok() {
        let res = mock_op_success().context("execute operation");
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), "success");
    }

    #[test]
    fn test_result_ext_context_on_err() {
        let res = mock_op_failure().context("authenticate request");

        assert!(res.is_err());
        let err_ctx = res.unwrap_err();
        assert_eq!(err_ctx.op.as_deref(), Some("authenticate request"));
        assert_matches!(err_ctx.source, Error::Unauthorized(_));
        assert!(err_ctx.details.is_none());
    }

    #[test]
    fn test_result_ext_implicit_from_conversion() {
        // Tests that ResultExt automatically runs `Into<Error>` on underlying sqlx::Error
        let res = mock_sqlx_failure().context("query DB");

        assert!(res.is_err());
        let err_ctx = res.unwrap_err();
        assert_eq!(err_ctx.op.as_deref(), Some("query DB"));
        assert_matches!(err_ctx.source, Error::NotFound { .. });
    }

    #[test]
    fn test_result_ext_with_context_lazy_evaluation() {
        let mut closure_called = false;

        // 1. On Ok, details closure MUST NOT be evaluated (zero overhead)
        let ok_res = mock_op_success().with_context("do something", || {
            closure_called = true;
            "computed detail"
        });
        assert!(ok_res.is_ok());
        assert!(!closure_called, "Closure should not be called on success");

        // 2. On Err, details closure MUST be evaluated
        let err_res = mock_op_failure().with_context("do something", || {
            closure_called = true;
            format!("request_id: {}", 99)
        });

        assert!(err_res.is_err());
        assert!(closure_called, "Closure should be called on error");
        let err_ctx = err_res.unwrap_err();
        assert_eq!(err_ctx.details.as_deref(), Some("request_id: 99"));
    }

    #[test]
    fn test_direct_question_mark_has_no_op() {
        let err = mock_direct_question_mark().unwrap_err();
        assert!(err.op.is_none());
        assert_matches!(err.source, Error::NotFound { .. });
    }

    // =====================================================================
    // 5. Size of Error and ErrorContext
    // =====================================================================

    #[test]
    fn test_sizes() {
        assert_eq!(
            std::mem::size_of::<Box<ErrorContext>>(),
            std::mem::size_of::<usize>()
        ); // It should always be the size of a pointer
        println!("ErrorContext: {}", std::mem::size_of::<ErrorContext>()); // Last test is 104 bytes
        println!("Error: {}", std::mem::size_of::<Error>()); // Last test is 56 bytes
    }
}
