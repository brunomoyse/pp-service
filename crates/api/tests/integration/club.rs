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
    let list_query = format!(r#"query {{ clubManagers(clubId: "{club_id}") {{ id email }} }}"#);
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
