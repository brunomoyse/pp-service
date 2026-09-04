use crate::common::*;
use api::gql::build_schema;

#[tokio::test]
async fn test_get_clubs_query() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let club_id = create_test_club(&app_state, "Test Poker Club").await;

    let query = r#"
        query {
            clubs {
                id
                name
                city
            }
        }
    "#;

    let response = execute_graphql(&schema, query, None, None).await;

    assert!(
        response.errors.is_empty(),
        "Clubs query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let clubs = data["clubs"].as_array().unwrap();

    assert!(!clubs.is_empty(), "Should return at least one club");

    // Find our test club
    let test_club = clubs.iter().find(|c| c["id"] == club_id.to_string());
    assert!(test_club.is_some(), "Should find our test club");
}

#[tokio::test]
async fn test_get_club_by_id() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let club_id = create_test_club(&app_state, "Specific Test Club").await;

    let query = r#"
        query {
            clubs {
                id
                name
                city
            }
        }
    "#;

    let response = execute_graphql(&schema, query, None, None).await;

    assert!(
        response.errors.is_empty(),
        "Clubs query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let clubs = data["clubs"].as_array().unwrap();

    // Find our specific test club
    let test_club = clubs
        .iter()
        .find(|c| c["id"] == club_id.to_string())
        .expect("Should find our test club");

    assert_eq!(test_club["id"], club_id.to_string());
    assert_eq!(test_club["name"], "Specific Test Club");
}

/// Regression: the ClubLoader's hand-written column list drifted from ClubRow
/// (missing address/vat_number/needs_review/plan/subscription columns), which
/// made every loader-resolved `club` field fail at runtime with
/// "no column found for name: address" while the repo queries kept working.
#[tokio::test]
async fn test_tournament_club_resolves_via_loader() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let club_id = create_test_club(&app_state, "Loader Club").await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Loader Tournament").await;

    let query =
        format!(r#"query {{ tournament(id: "{tournament_id}") {{ id club {{ id name }} }} }}"#);

    let response = execute_graphql(&schema, &query, None, None).await;

    assert!(
        response.errors.is_empty(),
        "tournament.club (ClubLoader) should resolve: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    assert_eq!(data["tournament"]["club"]["id"], club_id.to_string());
    assert_eq!(data["tournament"]["club"]["name"], "Loader Club");
}

/// Invite flow: a club manager invites a new email → account created (role
/// manager, no password), assignment active, team list shows both; the last
/// active manager cannot be revoked; players cannot invite.
#[tokio::test]
async fn test_invite_and_revoke_club_manager() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "invite_owner@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Invite Test Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    // Player must not be able to invite.
    let (_player_id, player_claims) =
        create_test_user(&app_state, "invite_player@test.com", "player").await;
    let invite = format!(
        r#"mutation {{ inviteClubManager(input: {{ clubId: "{club_id}", email: "coadmin@example.com", firstName: "Ana" }}) {{ createdAccount emailSent }} }}"#
    );
    let denied = execute_graphql(&schema, &invite, None, Some(player_claims)).await;
    assert!(!denied.errors.is_empty(), "player invite should be denied");

    // Manager invites a brand-new email → account is created.
    let response = execute_graphql(&schema, &invite, None, Some(manager_claims.clone())).await;
    assert!(
        response.errors.is_empty(),
        "invite should succeed: {:?}",
        response.errors
    );
    let data = response.data.into_json().unwrap();
    assert_eq!(data["inviteClubManager"]["createdAccount"], true);
    // emailSent depends on whether SCW_* creds are present in the test env —
    // just require the field to resolve.
    assert!(data["inviteClubManager"]["emailSent"].is_boolean());

    // Invited account exists with role manager and a pending set-password token.
    let row = sqlx::query(
        "SELECT id, role, password_hash FROM users WHERE email = 'coadmin@example.com'",
    )
    .fetch_one(&app_state.db)
    .await
    .unwrap();
    use sqlx::Row;
    let invited_id: uuid::Uuid = row.get("id");
    assert_eq!(row.get::<String, _>("role"), "manager");
    assert!(row.get::<Option<String>, _>("password_hash").is_none());
    let tokens: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM password_reset_tokens WHERE user_id = $1")
            .bind(invited_id)
            .fetch_one(&app_state.db)
            .await
            .unwrap();
    assert_eq!(tokens, 1);

    // Re-inviting the same email is idempotent (no duplicate assignment).
    let again = execute_graphql(&schema, &invite, None, Some(manager_claims.clone())).await;
    assert!(
        again.errors.is_empty(),
        "re-invite should succeed: {:?}",
        again.errors
    );

    // Team list shows both managers.
    let list_query =
        format!(r#"query {{ clubManagers(clubId: "{club_id}") {{ id email role }} }}"#);
    let list = execute_graphql(&schema, &list_query, None, Some(manager_claims.clone())).await;
    assert!(
        list.errors.is_empty(),
        "list should succeed: {:?}",
        list.errors
    );
    let list_data = list.data.into_json().unwrap();
    let managers = list_data["clubManagers"].as_array().unwrap();
    assert_eq!(managers.len(), 2, "both managers should be listed");
    let invited_assignment = managers
        .iter()
        .find(|m| m["email"] == "coadmin@example.com")
        .expect("invited manager listed");
    // An invite grants the lesser role unless one is asked for explicitly.
    assert_eq!(invited_assignment["role"], "MANAGER");
    let owner_assignment = managers
        .iter()
        .find(|m| m["email"] == "invite_owner@test.com")
        .expect("owner listed");
    assert_eq!(owner_assignment["role"], "OWNER");

    // Revoke the invited co-manager.
    let revoke = format!(
        r#"mutation {{ revokeClubManager(id: "{}") }}"#,
        invited_assignment["id"].as_str().unwrap()
    );
    let revoked = execute_graphql(&schema, &revoke, None, Some(manager_claims.clone())).await;
    assert!(
        revoked.errors.is_empty(),
        "revoke should succeed: {:?}",
        revoked.errors
    );

    // The last remaining manager cannot be removed.
    let list = execute_graphql(&schema, &list_query, None, Some(manager_claims.clone())).await;
    let list_data = list.data.into_json().unwrap();
    let managers = list_data["clubManagers"].as_array().unwrap();
    assert_eq!(managers.len(), 1);
    let last = format!(
        r#"mutation {{ revokeClubManager(id: "{}") }}"#,
        managers[0]["id"].as_str().unwrap()
    );
    let last_result = execute_graphql(&schema, &last, None, Some(manager_claims)).await;
    assert!(
        !last_result.errors.is_empty(),
        "removing the last manager must fail"
    );
}

/// Owner vs manager: a plain co-manager sees the team but cannot change it, and
/// a club can never be left without an owner.
#[tokio::test]
async fn test_club_manager_role_hierarchy() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());
    let stamp = chrono::Utc::now().timestamp_micros();

    let owner_email = format!("roles_owner_{stamp}@test.com");
    let (owner_id, owner_claims) = create_test_user(&app_state, &owner_email, "manager").await;
    let club_id = create_test_club(&app_state, "Roles Test Club").await;
    create_club_manager(&app_state, owner_id, club_id).await;

    let co_email = format!("roles_co_{stamp}@test.com");
    let (co_id, co_claims) = create_test_user(&app_state, &co_email, "manager").await;
    create_club_co_manager(&app_state, co_id, club_id).await;

    let list_query =
        format!(r#"query {{ clubManagers(clubId: "{club_id}") {{ id email role }} }}"#);
    let assignment_id = |resp: &serde_json::Value, email: &str| -> String {
        resp["clubManagers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["email"] == email)
            .unwrap_or_else(|| panic!("{email} should be listed"))["id"]
            .as_str()
            .unwrap()
            .to_string()
    };

    // Each side sees their own authority on `me`, which is what the apps gate on.
    let me_query = "query { me { clubRole } }";
    let owner_me = execute_graphql(&schema, me_query, None, Some(owner_claims.clone())).await;
    assert_eq!(
        owner_me.data.into_json().unwrap()["me"]["clubRole"],
        "OWNER"
    );
    let co_me = execute_graphql(&schema, me_query, None, Some(co_claims.clone())).await;
    assert_eq!(co_me.data.into_json().unwrap()["me"]["clubRole"], "MANAGER");

    // A co-manager may read the team...
    let list = execute_graphql(&schema, &list_query, None, Some(co_claims.clone())).await;
    assert!(
        list.errors.is_empty(),
        "co-manager should see the team: {:?}",
        list.errors
    );
    let list_data = list.data.into_json().unwrap();
    let owner_assignment = assignment_id(&list_data, &owner_email);
    let co_assignment = assignment_id(&list_data, &co_email);

    // ...but may not change it, nor the plan.
    let invite = format!(
        r#"mutation {{ inviteClubManager(input: {{ clubId: "{club_id}", email: "roles_new_{stamp}@example.com" }}) {{ createdAccount emailSent }} }}"#
    );
    let promote_co =
        format!(r#"mutation {{ setClubManagerRole(id: "{co_assignment}", role: OWNER) }}"#);
    let revoke_owner = format!(r#"mutation {{ revokeClubManager(id: "{owner_assignment}") }}"#);
    let redeem =
        format!(r#"mutation {{ redeemCode(clubId: "{club_id}", code: "NOPE") {{ id }} }}"#);

    for (label, mutation) in [
        ("invite", &invite),
        ("set role", &promote_co),
        ("revoke", &revoke_owner),
        ("redeem code", &redeem),
    ] {
        let denied = execute_graphql(&schema, mutation, None, Some(co_claims.clone())).await;
        assert!(
            !denied.errors.is_empty(),
            "a co-manager must not be able to {label}"
        );
    }

    // The owner promotes them, and now they can invite.
    let promoted = execute_graphql(&schema, &promote_co, None, Some(owner_claims.clone())).await;
    assert!(
        promoted.errors.is_empty(),
        "owner should be able to promote: {:?}",
        promoted.errors
    );
    let now_allowed = execute_graphql(&schema, &invite, None, Some(co_claims.clone())).await;
    assert!(
        now_allowed.errors.is_empty(),
        "a promoted owner should be able to invite: {:?}",
        now_allowed.errors
    );

    // With two owners, stepping one down is fine.
    let demote_owner =
        format!(r#"mutation {{ setClubManagerRole(id: "{owner_assignment}", role: MANAGER) }}"#);
    let demoted = execute_graphql(&schema, &demote_owner, None, Some(co_claims.clone())).await;
    assert!(
        demoted.errors.is_empty(),
        "demoting one of two owners should succeed: {:?}",
        demoted.errors
    );

    // The club is now down to a single owner, who can be neither demoted nor removed.
    let demote_last =
        format!(r#"mutation {{ setClubManagerRole(id: "{co_assignment}", role: MANAGER) }}"#);
    let refused = execute_graphql(&schema, &demote_last, None, Some(co_claims.clone())).await;
    assert!(
        !refused.errors.is_empty(),
        "the last owner must not be demotable"
    );

    let revoke_last_owner = format!(r#"mutation {{ revokeClubManager(id: "{co_assignment}") }}"#);
    let refused = execute_graphql(&schema, &revoke_last_owner, None, Some(co_claims)).await;
    assert!(
        !refused.errors.is_empty(),
        "the last owner must not be removable"
    );
}
