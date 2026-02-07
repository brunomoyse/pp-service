/// Unified error type for GraphQL resolvers.
///
/// async-graphql has a blanket `impl<T: Display + Send + Sync + 'static> From<T> for Error`,
/// so any type implementing `Display` auto-converts via `?`.
///
/// This enum gives us:
///   - `From<sqlx::Error>` — logs the DB detail, shows a sanitized message to clients
///   - `From<uuid::Error>` — shows "Invalid ID: …"
///   - `From<serde_json::Error>` — shows "Serialization error: …"
///   - `GqlError::new("…")` — custom one-off messages
#[derive(Debug)]
pub enum GqlError {
    Sqlx(sqlx::Error),
    Uuid(uuid::Error),
    SerdeJson(serde_json::Error),
    Custom(String),
}

impl GqlError {
    pub fn new(msg: impl Into<String>) -> Self {
        GqlError::Custom(msg.into())
    }
}

impl std::fmt::Display for GqlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GqlError::Sqlx(e) => {
                // Log the real error server-side; return a generic message to clients
                tracing::error!("Database error: {e}");
                write!(f, "Internal database error")
            }
            GqlError::Uuid(e) => write!(f, "Invalid ID: {e}"),
            GqlError::SerdeJson(e) => write!(f, "Serialization error: {e}"),
            GqlError::Custom(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for GqlError {}

impl From<sqlx::Error> for GqlError {
    fn from(e: sqlx::Error) -> Self {
        GqlError::Sqlx(e)
    }
}

impl From<uuid::Error> for GqlError {
    fn from(e: uuid::Error) -> Self {
        GqlError::Uuid(e)
    }
}

impl From<serde_json::Error> for GqlError {
    fn from(e: serde_json::Error) -> Self {
        GqlError::SerdeJson(e)
    }
}

/// Extension trait that converts any `Result<T, E>` where `E: Display`
/// into `async_graphql::Result<T>` with a contextual message prefix.
///
/// Usage: `Uuid::parse_str(id).gql_err("Invalid tournament ID")?`
pub trait ResultExt<T> {
    fn gql_err(self, context: &str) -> std::result::Result<T, async_graphql::Error>;
}

impl<T, E: std::fmt::Display> ResultExt<T> for std::result::Result<T, E> {
    fn gql_err(self, context: &str) -> std::result::Result<T, async_graphql::Error> {
        self.map_err(|e| async_graphql::Error::new(format!("{context}: {e}")))
    }
}
