// The merged GraphQL schema generates deeply-nested async future types; as the
// domain count grew, laying them out overflowed the default 128 recursion limit.
#![recursion_limit = "512"]

pub mod app;
pub mod auth;
pub mod error;
pub mod features;
pub mod gql;
pub mod middleware;
pub mod routes;
pub mod services;
pub mod state;

pub use state::AppState;
