// Modular domain-based architecture
pub mod common;
pub mod domains;
pub mod root;

// Shared infrastructure
pub mod error;
pub mod loaders;
pub mod scalars;
pub mod schema;
pub mod subscriptions;
pub mod types;

// Re-exports
pub use root::{MutationRoot, QueryRoot};
pub use schema::build_schema;
pub use subscriptions::SubscriptionRoot;
