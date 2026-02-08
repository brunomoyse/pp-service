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

// Legacy modules (kept for backward compatibility, but unused in new architecture)
pub mod mutations;
pub mod queries;
pub mod tournament_clock;

// Re-exports - using new modular root resolvers
pub use root::{MutationRoot, QueryRoot};
pub use schema::build_schema;
pub use subscriptions::SubscriptionRoot;
