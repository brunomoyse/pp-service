use async_graphql::{Context, Error, Result};
use uuid::Uuid;
use crate::auth::Claims;
use crate::state::AppState;
use crate::gql::types::{User, Role};
use infra::repos::ClubManagerRepo;

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

/// Check if the authenticated user is a manager for a specific club
pub async fn require_club_manager(ctx: &Context<'_>, club_id: Uuid) -> Result<User> {
    let user = require_role(ctx, Role::Manager).await?;
    
    // Admin can manage any club
    if user.role == Role::Admin {
        return Ok(user);
    }
    
    let state = ctx.data::<AppState>()?;
    let club_manager_repo = ClubManagerRepo::new(state.db.clone());
    
    let user_id = Uuid::parse_str(user.id.as_str())
        .map_err(|e| Error::new(format!("Invalid user ID: {}", e)))?;
    
    let is_manager = club_manager_repo
        .is_club_manager(user_id, club_id)
        .await
        .map_err(|e| Error::new(format!("Database error: {}", e)))?;
    
    if !is_manager {
        return Err(Error::new("You are not authorized to manage this club"));
    }
    
    Ok(user)
}

/// Check if the authenticated user is an admin (global access)
#[allow(dead_code)]
pub async fn require_admin(ctx: &Context<'_>) -> Result<User> {
    require_role(ctx, Role::Admin).await
}

fn has_required_role(user_role: &Role, required_role: Role) -> bool {
    match required_role {
        Role::Admin => *user_role == Role::Admin,
        Role::Manager => *user_role == Role::Manager || *user_role == Role::Admin, // Admin has manager permissions
        Role::Player => true, // Everyone has player permissions
    }
}