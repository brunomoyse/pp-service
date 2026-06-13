// The merged GraphQL schema generates deeply-nested async future types; laying
// them out (e.g. when building the schema in tests) overflows the default limit.
#![recursion_limit = "512"]

mod common;

mod auth;
mod check_in;
mod clock_lifecycle;
mod club;
mod club_tables;
mod data_retention;
mod drinks;
mod economy_firewall;
mod eliminate_player;
mod money_reconciliation;
mod notification;
mod payouts;
mod permission;
mod player_management;
mod query_coverage;
mod refresh_token_security;
mod system;
mod table_seating;
mod tournament;
mod tournament_clock;
mod tournament_entries;
mod tournament_results;
mod unassign_table;
mod user;
