use async_graphql::{Context, Error, Result};
use uuid::Uuid;
use crate::auth::Claims;
use crate::state::AppState;
use crate::gql::types::{User, Role};

/// Check if the authenticated user has the required role
pub async fn require_role(ctx: &Context<'_>, required_role: Role) -> Result<User> {
    let claims = ctx.data::<Claims>()
        .map_err(|_| Error::new("Authentication required"))?;
    
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| Error::new(format!("Invalid user ID: {}", e)))?;
    
    let state = ctx.data::<AppState>()?;
    let user = get_user_by_id_with_role(state, user_id).await?;
    
    if !has_required_role(&user.role, required_role) {
        return Err(Error::new(format!(
            "Insufficient permissions. Required role: {:?}",
            required_role
        )));
    }
    
    Ok(user)
}

/// Check if the authenticated user has admin permissions when a condition is met
pub async fn require_admin_if(ctx: &Context<'_>, condition: bool, _field_name: &str) -> Result<Option<User>> {
    if condition {
        let admin_user = require_role(ctx, Role::Manager).await?;
        Ok(Some(admin_user))
    } else {
        // Still need to get the authenticated user for normal operations
        let claims = ctx.data::<Claims>()
            .map_err(|_| Error::new("Authentication required"))?;
        
        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|e| Error::new(format!("Invalid user ID: {}", e)))?;
        
        let state = ctx.data::<AppState>()?;
        let user = get_user_by_id_with_role(state, user_id).await?;
        Ok(Some(user))
    }
}

async fn get_user_by_id_with_role(state: &AppState, user_id: Uuid) -> Result<User> {
    let row = sqlx::query!(
        "SELECT id, email, username, first_name, last_name, phone, is_active, role FROM users WHERE id = $1",
        user_id
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| Error::new(e.to_string()))?;

    Ok(User {
        id: row.id.into(),
        email: row.email,
        username: row.username,
        first_name: row.first_name,
        last_name: row.last_name,
        phone: row.phone,
        is_active: row.is_active,
        role: crate::gql::types::Role::from(row.role),
    })
}

fn has_required_role(user_role: &Role, required_role: Role) -> bool {
    match required_role {
        Role::Manager => *user_role == Role::Manager,
        Role::Player => true, // Everyone has player permissions
    }
}