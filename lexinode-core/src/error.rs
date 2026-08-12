use std::borrow::Cow;

/// Type alias for `Result<T, Box<ErrorContext<E>>>`.
///
/// Generic over the underlying domain error type `E`. Each crate (e.g. `backend`,
/// `cli`) defines its own domain `Error` enum and its own `Result<T>` alias that
/// pins `E` to that type, while sharing this `ErrorContext`/`ResultExt` machinery.
pub type Result<T, E> = std::result::Result<T, Box<ErrorContext<E>>>;

/// Convenience alias for `Cow<'static, str>`.
///
/// Allows accepting both zero-cost static string literals (`&'static str`)
/// and dynamically allocated strings (`String`).
type CowStr = Cow<'static, str>;

/// Rich operational wrapper that attaches context to an underlying domain error `E`.
///
/// Designed to provide meaningful error messages by pairing what action was being
/// performed (`op`) with the root error (`source`) and optional debug context (`details`).
#[derive(Debug)]
pub struct ErrorContext<E> {
    /// High-level description of the operation being executed (e.g., "fetch user profile").
    /// `None` when the error was produced via a bare `?` without `.context(...)`.
    pub op: Option<CowStr>,
    /// The underlying domain error trigger.
    pub source: E,
    /// Optional structured/unstructured metadata providing deeper diagnostic context.
    pub details: Option<CowStr>,
}

impl<E: std::fmt::Display> std::fmt::Display for ErrorContext<E> {
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

impl<E: std::error::Error + 'static> std::error::Error for ErrorContext<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Allows a bare `?` to convert any domain error `E` directly into
/// `Box<ErrorContext<E>>` without requiring an explicit `.context(...)` call.
/// The resulting `ErrorContext` has `op: None`.
impl<E> From<E> for Box<ErrorContext<E>> {
    fn from(err: E) -> Self {
        Box::new(ErrorContext {
            op: None,
            source: err,
            details: None,
        })
    }
}

/// Extension trait for [`std::result::Result`] providing fluent contextual error binding.
///
/// Allows chaining context onto any `Result<T, F>` where `F` can be converted into
/// the target domain error type `E` (including the trivial case `F == E`).
pub trait ResultExt<T, E> {
    /// Wraps an error with an operation name.
    ///
    /// # Arguments
    ///
    /// * `op` - High-level description of the operation being attempted.
    fn context(self, op: impl Into<CowStr>) -> Result<T, E>;

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
    ) -> Result<T, E>;
}

impl<T, E, F> ResultExt<T, E> for std::result::Result<T, F>
where
    F: Into<E>,
{
    fn context(self, op: impl Into<CowStr>) -> Result<T, E> {
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
    ) -> Result<T, E> {
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

    #[derive(Debug, thiserror::Error)]
    #[error("test error")]
    struct TestError;

    #[derive(Debug, thiserror::Error)]
    #[error("test error with resource {resource} and id {id}")]
    struct TestResourceError {
        resource: CowStr,
        id: CowStr,
    }

    /// A "lower-level" error type distinct from `TestError`, used to verify that
    /// `.context()` performs a real `F -> E` conversion (not just `E -> E`).
    #[derive(Debug, thiserror::Error)]
    #[error("low-level io failure")]
    struct LowLevelError;

    /// Domain error that can be built from `LowLevelError`, mirroring how a crate's
    /// real `Error` enum would wrap e.g. `sqlx::Error` or `reqwest::Error`.
    #[derive(Debug, thiserror::Error)]
    enum DomainError {
        #[error("low level: {0}")]
        LowLevel(#[from] LowLevelError),
        #[error("resource error: {0}")]
        Resource(#[from] TestResourceError),
    }

    // =====================================================================
    // 1. ErrorContext & Display Tests
    // =====================================================================

    #[test]
    fn test_error_context_display_without_details() {
        let ctx = ErrorContext {
            op: Some("test op".into()),
            source: TestError,
            details: None,
        };

        assert_eq!(ctx.to_string(), "Failed to test op: test error");
    }

    #[test]
    fn test_error_context_display_without_op() {
        let ctx = ErrorContext {
            op: None,
            source: TestError,
            details: None,
        };

        assert_eq!(ctx.to_string(), "test error");
    }

    #[test]
    fn test_error_context_display_with_details() {
        let ctx = ErrorContext {
            op: Some("test op".into()),
            source: TestError,
            details: Some("test details".into()),
        };

        assert_eq!(
            ctx.to_string(),
            "Failed to test op: test error (details: test details)"
        );
    }

    #[test]
    fn test_std_error_source_chain() {
        use std::error::Error as StdError;

        let ctx = ErrorContext {
            op: Some("test op".into()),
            source: TestError,
            details: None,
        };

        // Verify std::error::Error::source properly returns the underlying domain error
        let source = ctx.source().expect("should have source error");
        assert_eq!(source.to_string(), "test error");
    }

    // =====================================================================
    // 2. ResultExt Extension Trait Tests
    // =====================================================================

    #[allow(clippy::unnecessary_wraps)]
    fn mock_op_success() -> std::result::Result<&'static str, TestError> {
        Ok("success")
    }

    fn mock_op_failure() -> std::result::Result<&'static str, TestError> {
        Err(TestError)
    }

    fn mock_op_failure_with_resource() -> std::result::Result<&'static str, TestResourceError> {
        Err(TestResourceError {
            resource: "test".into(),
            id: "123".into(),
        })
    }

    fn mock_low_level_failure() -> std::result::Result<&'static str, LowLevelError> {
        Err(LowLevelError)
    }

    fn mock_direct_question_mark() -> Result<&'static str, TestResourceError> {
        // Check that a bare `?` converts to `Box<ErrorContext<_>>` without `op`.
        let _ = mock_op_failure_with_resource()?;
        Ok("unreachable")
    }

    #[test]
    fn test_result_ext_context_on_ok() {
        let res: Result<&'static str, TestError> = mock_op_success().context("test context");
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), "success");
    }

    #[test]
    fn test_result_ext_context_on_err() {
        let res = mock_op_failure().context("test context");

        assert!(res.is_err());
        let err_ctx = res.unwrap_err();
        assert_eq!(err_ctx.op.as_deref(), Some("test context"));
        assert_matches!(err_ctx.source, TestError);
        assert!(err_ctx.details.is_none());
    }

    /// This is the test that actually exercises `F: Into<E>` where `F != E`:
    /// `LowLevelError` gets converted into `DomainError` via `.context(...)`.
    #[test]
    fn test_result_ext_cross_type_conversion() {
        let res: Result<&'static str, DomainError> =
            mock_low_level_failure().context("read config file");

        assert!(res.is_err());
        let err_ctx = res.unwrap_err();
        assert_eq!(err_ctx.op.as_deref(), Some("read config file"));
        assert_matches!(err_ctx.source, DomainError::LowLevel(_));
    }

    #[test]
    fn test_result_ext_same_type_passthrough() {
        // F == E: still works via the reflexive `impl<T> From<T> for T`.
        let res = mock_op_failure_with_resource().context("test context");

        assert!(res.is_err());
        let err_ctx = res.unwrap_err();
        assert_eq!(err_ctx.op.as_deref(), Some("test context"));
        assert_matches!(err_ctx.source, TestResourceError { .. });
    }

    #[test]
    fn test_result_ext_with_context_lazy_evaluation() {
        let mut closure_called = false;

        // 1. On Ok, details closure MUST NOT be evaluated (zero overhead)
        let ok_res: Result<&'static str, TestError> =
            mock_op_success().with_context("do something", || {
                closure_called = true;
                "computed detail"
            });
        assert!(ok_res.is_ok());
        assert!(!closure_called, "Closure should not be called on success");

        // 2. On Err, details closure MUST be evaluated
        let err_res: Result<&'static str, TestError> =
            mock_op_failure().with_context("do something", || {
                closure_called = true;
                format!("request_id: {}", 123)
            });

        assert!(err_res.is_err());
        assert!(closure_called, "Closure should be called on error");
        let err_ctx = err_res.unwrap_err();
        assert_eq!(err_ctx.details.as_deref(), Some("request_id: 123"));
    }

    #[test]
    fn test_direct_question_mark_has_no_op() {
        let err = mock_direct_question_mark().unwrap_err();
        assert!(err.op.is_none());
        assert_matches!(err.source, TestResourceError { .. });
    }

    // =====================================================================
    // 3. Size of ErrorContext
    // =====================================================================

    #[test]
    fn test_sizes() {
        use std::mem::size_of;

        assert_eq!(
            size_of::<Box<ErrorContext<TestError>>>(),
            size_of::<usize>(),
            "`Box<ErrorContext<TestError>>` should always be the size of a pointer"
        );

        assert_eq!(
            size_of::<Box<ErrorContext<TestResourceError>>>(),
            size_of::<usize>(),
            "`Box<ErrorContext<TestResourceError>>` should always be the size of a pointer"
        );

        assert_eq!(
            size_of::<Box<ErrorContext<DomainError>>>(),
            size_of::<usize>(),
            "`Box<ErrorContext<DomainError>>` should always be the size of a pointer"
        );
    }
}
