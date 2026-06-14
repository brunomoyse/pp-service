use crate::common::*;
use api::gql::build_schema;
use async_graphql::Variables;
use serde_json::json;
use uuid::Uuid;

const CREATE: &str = r#"
    mutation Create($input: CreateAnnouncementInput!) {
        createAnnouncement(input: $input) {
            id
            scope
            title
            clubId
            tournamentId
        }
    }
"#;

const FEED: &str = r#"
    query Feed {
        myAnnouncements {
            totalCount
            items { id title scope }
        }
    }
"#;

/// Link an app user to a club's roster as a claimed, active member.
async fn add_claimed_roster(app_state: &api::state::AppState, club_id: Uuid, app_user_id: Uuid) {
    sqlx::query(
        "INSERT INTO club_player (club_id, display_name, app_user_id, is_active) \
         VALUES ($1, $2, $3, true)",
    )
    .bind(club_id)
    .bind("Roster Member")
    .bind(app_user_id)
    .execute(&app_state.db)
    .await
    .expect("Failed to add claimed roster entry");
}

fn feed_titles(resp: &async_graphql::Response) -> Vec<String> {
    let data = resp.data.clone().into_json().unwrap();
    data["myAnnouncements"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["title"].as_str().unwrap().to_string())
        .collect()
}

/// A CLUB announcement reaches the club's roster members and only them.
#[tokio::test]
async fn test_club_announcement_targets_roster_only() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "ann_club_mgr@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Announcement Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let (member_id, member_claims) =
        create_test_user(&app_state, "ann_member@test.com", "player").await;
    add_claimed_roster(&app_state, club_id, member_id).await;

    let (_outsider_id, outsider_claims) =
        create_test_user(&app_state, "ann_outsider@test.com", "player").await;

    // Manager creates a club-wide announcement.
    let vars = Variables::from_json(json!({
        "input": {
            "scope": "CLUB",
            "clubId": club_id.to_string(),
            "title": "Five years with GGPoker",
            "body": "Thanks for being part of the journey."
        }
    }));
    let resp = execute_graphql(&schema, CREATE, Some(vars), Some(manager_claims)).await;
    assert!(resp.errors.is_empty(), "create club ann: {:?}", resp.errors);
    let data = resp.data.into_json().unwrap();
    assert_eq!(data["createAnnouncement"]["scope"], "CLUB");

    // The roster member sees it.
    let resp = execute_graphql(&schema, FEED, None, Some(member_claims)).await;
    assert!(resp.errors.is_empty(), "member feed: {:?}", resp.errors);
    assert!(feed_titles(&resp).contains(&"Five years with GGPoker".to_string()));

    // The outsider does not.
    let resp = execute_graphql(&schema, FEED, None, Some(outsider_claims)).await;
    assert!(resp.errors.is_empty(), "outsider feed: {:?}", resp.errors);
    assert!(!feed_titles(&resp).contains(&"Five years with GGPoker".to_string()));
}

/// A TOURNAMENT announcement reaches registered players only.
#[tokio::test]
async fn test_tournament_announcement_targets_registrants() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "ann_trn_mgr@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Pizza Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Pizza Night").await;

    let (registrant_id, registrant_claims) =
        create_test_user(&app_state, "ann_registrant@test.com", "player").await;
    create_test_registration(&app_state, tournament_id, registrant_id, "registered").await;

    let (_other_id, other_claims) =
        create_test_user(&app_state, "ann_unregistered@test.com", "player").await;

    let vars = Variables::from_json(json!({
        "input": {
            "scope": "TOURNAMENT",
            "tournamentId": tournament_id.to_string(),
            "title": "Free pizza tonight",
            "body": "Grab a slice at the bar."
        }
    }));
    let resp = execute_graphql(&schema, CREATE, Some(vars), Some(manager_claims)).await;
    assert!(resp.errors.is_empty(), "create trn ann: {:?}", resp.errors);
    let data = resp.data.into_json().unwrap();
    // Tournament scope derives the club from the tournament.
    assert_eq!(data["createAnnouncement"]["clubId"], club_id.to_string());

    let resp = execute_graphql(&schema, FEED, None, Some(registrant_claims)).await;
    assert!(feed_titles(&resp).contains(&"Free pizza tonight".to_string()));

    let resp = execute_graphql(&schema, FEED, None, Some(other_claims)).await;
    assert!(!feed_titles(&resp).contains(&"Free pizza tonight".to_string()));
}

/// PLATFORM announcements require an admin and reach every player.
#[tokio::test]
async fn test_platform_announcement_requires_admin() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (_mgr_id, manager_claims) =
        create_test_user(&app_state, "ann_plat_mgr@test.com", "manager").await;
    let (_admin_id, admin_claims) =
        create_test_user(&app_state, "ann_admin@test.com", "admin").await;
    let (_player_id, player_claims) =
        create_test_user(&app_state, "ann_plat_player@test.com", "player").await;

    let make_vars = || {
        Variables::from_json(json!({
            "input": {
                "scope": "PLATFORM",
                "title": "PocketPair 2.0 is here",
                "body": "A fresh new look across the app."
            }
        }))
    };

    // A non-admin manager is rejected.
    let resp = execute_graphql(&schema, CREATE, Some(make_vars()), Some(manager_claims)).await;
    assert!(
        !resp.errors.is_empty(),
        "manager should not create a platform announcement"
    );

    // An admin succeeds.
    let resp = execute_graphql(&schema, CREATE, Some(make_vars()), Some(admin_claims)).await;
    assert!(
        resp.errors.is_empty(),
        "admin platform ann: {:?}",
        resp.errors
    );

    // Every player sees it.
    let resp = execute_graphql(&schema, FEED, None, Some(player_claims)).await;
    assert!(feed_titles(&resp).contains(&"PocketPair 2.0 is here".to_string()));
}

/// The announcements notification preference round-trips through GraphQL.
#[tokio::test]
async fn test_announcement_preference_toggle() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (_player_id, player_claims) =
        create_test_user(&app_state, "ann_pref_player@test.com", "player").await;

    let update = r#"
        mutation Update($input: UpdateNotificationPreferencesInput!) {
            updateNotificationPreferences(input: $input) { announcements }
        }
    "#;
    let vars = Variables::from_json(json!({ "input": { "announcements": false } }));
    let resp = execute_graphql(&schema, update, Some(vars), Some(player_claims.clone())).await;
    assert!(resp.errors.is_empty(), "update prefs: {:?}", resp.errors);
    let data = resp.data.into_json().unwrap();
    assert_eq!(
        data["updateNotificationPreferences"]["announcements"],
        false
    );

    let query = r#"query { myNotificationPreferences { announcements } }"#;
    let resp = execute_graphql(&schema, query, None, Some(player_claims)).await;
    let data = resp.data.into_json().unwrap();
    assert_eq!(data["myNotificationPreferences"]["announcements"], false);
}
