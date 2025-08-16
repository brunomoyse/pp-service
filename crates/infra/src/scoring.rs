/// Tournament scoring utility with authoritative formula
/// 
/// Formula: points = min(60, round(3 * (sqrt(field_size) / sqrt(rank)) * (log10(buy_in_eur) + 1) + 2))
/// 
/// # Arguments
/// 
/// * `field_size` - Number of confirmed entrants (N ≥ 1)
/// * `rank` - Final position (1 = winner)
/// * `buy_in_eur` - Buy-in amount in euros (> 0)
/// 
/// # Returns
/// 
/// Points awarded (0-60), or 0 if inputs invalid
/// 
/// # Examples
/// 
/// ```
/// use infra::scoring::event_points;
/// 
/// assert_eq!(event_points(40, 1, 20.0), 46);
/// assert_eq!(event_points(50, 2, 30.0), 39);
/// assert_eq!(event_points(80, 9, 50.0), 26);
/// ```
pub fn event_points(field_size: u32, rank: u32, buy_in_eur: f64) -> u32 {
    // Guardrails: validate inputs
    if field_size < 1 || rank < 1 || rank > field_size || buy_in_eur <= 0.0 {
        return 0;
    }
    
    // Convert to f64 for calculations
    let field_size_f = field_size as f64;
    let rank_f = rank as f64;
    
    // Formula: 3 * (sqrt(field_size) / sqrt(rank)) * (log10(buy_in_eur) + 1) + 2
    let sqrt_field = field_size_f.sqrt();
    let sqrt_rank = rank_f.sqrt();
    let log_buy_in = buy_in_eur.log10();
    
    let raw_points = 3.0 * (sqrt_field / sqrt_rank) * (log_buy_in + 1.0) + 2.0;
    
    // Ensure points are non-negative, round, and cap at 60
    if raw_points <= 0.0 {
        return 0;
    }
    
    let rounded_points = raw_points.round() as u32;
    std::cmp::min(60, rounded_points)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_points_basic_cases() {
        // Test cases from specification
        assert_eq!(event_points(40, 1, 20.0), 46);
        assert_eq!(event_points(50, 2, 30.0), 39);
        assert_eq!(event_points(80, 9, 50.0), 26);
    }

    #[test]
    fn test_event_points_edge_cases() {
        // Minimum valid inputs - let's calculate what this should actually be
        // field_size=1, rank=1, buy_in=0.01
        // 3 * (sqrt(1) / sqrt(1)) * (log10(0.01) + 1) + 2
        // = 3 * (1 / 1) * (-2 + 1) + 2 
        // = 3 * 1 * (-1) + 2
        // = -3 + 2 = -1, rounded = 0 (because we can't have negative points)
        let small_points = event_points(1, 1, 0.01);
        assert_eq!(small_points, 0); // Should be 0 due to negative calculation
        
        // Better minimum case
        assert_eq!(event_points(1, 1, 1.0), 5); // log10(1) = 0, so 3*1*1 + 2 = 5
        
        // Large tournament, first place
        assert_eq!(event_points(200, 1, 100.0), 60); // Should cap at 60
        
        // Last place in small tournament
        let last_place_points = event_points(10, 10, 10.0);
        assert!(last_place_points > 0); // Should be positive
    }

    #[test]
    fn test_event_points_invalid_inputs() {
        // Invalid field size
        assert_eq!(event_points(0, 1, 10.0), 0);
        
        // Invalid rank (0)
        assert_eq!(event_points(10, 0, 10.0), 0);
        
        // Invalid rank (greater than field size)
        assert_eq!(event_points(10, 11, 10.0), 0);
        
        // Invalid buy-in (0)
        assert_eq!(event_points(10, 1, 0.0), 0);
        
        // Invalid buy-in (negative)
        assert_eq!(event_points(10, 1, -5.0), 0);
    }

    #[test]
    fn test_event_points_formula_components() {
        // Test that winners get more points than non-winners
        let winner_points = event_points(50, 1, 25.0);
        let second_points = event_points(50, 2, 25.0);
        let tenth_points = event_points(50, 10, 25.0);
        
        assert!(winner_points > second_points);
        assert!(second_points > tenth_points);
        
        // Test that larger field sizes generally give more points for same position
        let small_field = event_points(20, 1, 25.0);
        let large_field = event_points(100, 1, 25.0);
        
        assert!(large_field > small_field);
        
        // Test that higher buy-ins give more points for same position/field
        let low_buyin = event_points(50, 1, 10.0);
        let high_buyin = event_points(50, 1, 100.0);
        
        assert!(high_buyin > low_buyin);
    }

    #[test]
    fn test_event_points_cap() {
        // Test that points are capped at 60
        let max_points = event_points(1000, 1, 1000.0);
        assert_eq!(max_points, 60);
    }
}