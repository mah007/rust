//! Route registration errors.

use std::fmt;

/// An error produced while registering routes.
///
/// These are *startup* errors: they surface when the application is built, not
/// while serving requests, so misconfigured routing fails fast and loudly.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RouteError {
    /// Two routes conflict — for example the same method+path registered twice,
    /// or two differently-named parameters at the same position.
    Conflict {
        /// The path being registered when the conflict was detected.
        path: String,
        /// A human-readable description of the conflict.
        message: String,
    },
    /// A route pattern used invalid parameter or wildcard syntax.
    InvalidPattern {
        /// The offending path pattern.
        path: String,
        /// A human-readable description of the problem.
        message: String,
    },
}

impl RouteError {
    pub(crate) fn conflict(path: impl Into<String>, message: impl Into<String>) -> Self {
        RouteError::Conflict {
            path: path.into(),
            message: message.into(),
        }
    }

    pub(crate) fn invalid_pattern(path: impl Into<String>, message: impl Into<String>) -> Self {
        RouteError::InvalidPattern {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for RouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RouteError::Conflict { path, message } => {
                write!(f, "route conflict for `{path}`: {message}")
            }
            RouteError::InvalidPattern { path, message } => {
                write!(f, "invalid route pattern `{path}`: {message}")
            }
        }
    }
}

impl std::error::Error for RouteError {}
