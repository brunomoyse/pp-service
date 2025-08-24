use infra::repos::ClubTableRepo;
use sqlx::PgPool;
use std::env;

#[tokio::test]
async fn test_club_tables_system() {
    let database_url =
        env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL must be set for integration tests");

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    let club_table_repo = ClubTableRepo::new(pool.clone());

    // Test that we can retrieve Poker One club tables
    let poker_one_club_id = uuid::Uuid::parse_str("66666666-6666-6666-6666-666666666666").unwrap();

    // Get all tables for Poker One club
    let tables = club_table_repo
        .get_by_club(poker_one_club_id)
        .await
        .expect("Should be able to get club tables");

    println!("Found {} tables for Poker One club", tables.len());

    // Verify we have the 4 tables we created in the seeder
    assert_eq!(tables.len(), 4, "Should have 4 tables for Poker One");

    // Verify table numbers are 1, 2, 3, 4
    let table_numbers: Vec<i32> = tables.iter().map(|t| t.table_number).collect();
    assert_eq!(table_numbers, vec![1, 2, 3, 4]);

    // Test table details
    let table1 = &tables[0]; // Table 1
    assert_eq!(table1.table_number, 1);
    assert_eq!(table1.max_seats, 9);
    assert_eq!(table1.table_name, Some("Main Table 1".to_string()));
    assert_eq!(table1.location, Some("Center room".to_string()));

    let final_table = &tables[3]; // Table 4 - Final Table
    assert_eq!(final_table.table_number, 4);
    assert_eq!(final_table.max_seats, 6);
    assert_eq!(final_table.table_name, Some("Final Table".to_string()));
    assert_eq!(final_table.location, Some("VIP area".to_string()));

    // Test get available tables (should return all 4 since none are assigned)
    let available_tables = club_table_repo
        .get_available_by_club(poker_one_club_id)
        .await
        .expect("Should be able to get available tables");

    assert_eq!(available_tables.len(), 4, "All tables should be available");

    println!("✅ Club tables system test passed!");

    // Test table assignment to a tournament
    let tournament_id = uuid::Uuid::parse_str("10004444-4444-4444-4444-444444444444").unwrap(); // Thursday Live Event
    let table1_id = tables[0].id;

    // Assign table 1 to the tournament
    let assignment = club_table_repo
        .assign_to_tournament(tournament_id, table1_id)
        .await
        .expect("Should be able to assign table to tournament");

    println!(
        "✅ Assigned table {} to tournament",
        assignment.club_table_id
    );

    // Verify available tables is now 3 (one assigned)
    let available_after_assignment = club_table_repo
        .get_available_by_club(poker_one_club_id)
        .await
        .expect("Should be able to get available tables after assignment");

    assert_eq!(
        available_after_assignment.len(),
        3,
        "Should have 3 available tables after assignment"
    );

    // Get assigned tables for the tournament
    let assigned_tables = club_table_repo
        .get_assigned_to_tournament(tournament_id)
        .await
        .expect("Should be able to get assigned tables");

    assert_eq!(assigned_tables.len(), 1, "Should have 1 assigned table");
    assert_eq!(assigned_tables[0].id, table1_id);

    println!("✅ Table assignment test passed!");
}
