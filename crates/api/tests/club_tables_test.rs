mod common;

use common::*;
use infra::repos::club_tables;

#[tokio::test]
async fn test_club_tables_system() {
    let app_state = setup_test_db().await;
    let pool = &app_state.db;

    // Create test club and tables directly
    let poker_one_club_id = uuid::Uuid::new_v4();

    // Create test club
    sqlx::query!(
        "INSERT INTO clubs (id, name, city) VALUES ($1, $2, $3)",
        poker_one_club_id,
        "Test Poker Club",
        "Test City"
    )
    .execute(pool)
    .await
    .expect("Should be able to create test club");

    // Create 4 test tables
    for i in 1..=4 {
        let table_id = uuid::Uuid::new_v4();
        let max_seats = if i == 4 { 6 } else { 9 }; // Table 4 is the final table with 6 seats

        sqlx::query!(
            "INSERT INTO club_tables (id, club_id, table_number, max_seats) VALUES ($1, $2, $3, $4)",
            table_id,
            poker_one_club_id,
            i as i32,
            max_seats
        )
        .execute(pool)
        .await
        .expect("Should be able to create test table");
    }

    // Get all tables for test club
    let tables = club_tables::list_by_club(pool, poker_one_club_id)
        .await
        .expect("Should be able to get club tables");

    println!("Found {} tables for test club", tables.len());

    // Verify we have the 4 tables we created
    assert_eq!(tables.len(), 4, "Should have 4 tables for test club");

    // Verify table numbers are 1, 2, 3, 4
    let table_numbers: Vec<i32> = tables.iter().map(|t| t.table_number).collect();
    assert_eq!(table_numbers, vec![1, 2, 3, 4]);

    // Test table details
    let table1 = &tables[0]; // Table 1
    assert_eq!(table1.table_number, 1);
    assert_eq!(table1.max_seats, 9);

    let final_table = &tables[3]; // Table 4 - Final Table
    assert_eq!(final_table.table_number, 4);
    assert_eq!(final_table.max_seats, 6);

    // Test get available tables (should return all 4 since none are assigned)
    let available_tables = club_tables::list_available_by_club(pool, poker_one_club_id)
        .await
        .expect("Should be able to get available tables");

    assert_eq!(available_tables.len(), 4, "All tables should be available");

    println!("✅ Club tables system test passed!");

    // Create test tournament for assignment test
    let tournament_id = uuid::Uuid::new_v4();
    sqlx::query!(
        r#"INSERT INTO tournaments (
            id, name, description, club_id, start_time, end_time, 
            buy_in_cents, seat_cap, live_status
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'not_started'::tournament_live_status)"#,
        tournament_id,
        "Test Tournament",
        "Test tournament description",
        poker_one_club_id,
        chrono::Utc::now(),
        chrono::Utc::now() + chrono::Duration::hours(4),
        5000i32,
        100i32
    )
    .execute(pool)
    .await
    .expect("Should be able to create test tournament");

    let table1_id = tables[0].id;

    // Assign table 1 to the tournament
    let assignment = club_tables::assign_to_tournament(pool, tournament_id, table1_id)
        .await
        .expect("Should be able to assign table to tournament");

    println!(
        "✅ Assigned table {} to tournament",
        assignment.club_table_id
    );

    // Verify available tables is now 3 (one assigned)
    let available_after_assignment = club_tables::list_available_by_club(pool, poker_one_club_id)
        .await
        .expect("Should be able to get available tables after assignment");

    assert_eq!(
        available_after_assignment.len(),
        3,
        "Should have 3 available tables after assignment"
    );

    // Get assigned tables for the tournament
    let assigned_tables = club_tables::list_assigned_to_tournament(pool, tournament_id)
        .await
        .expect("Should be able to get assigned tables");

    assert_eq!(assigned_tables.len(), 1, "Should have 1 assigned table");
    assert_eq!(assigned_tables[0].id, table1_id);

    println!("✅ Table assignment test passed!");
}
