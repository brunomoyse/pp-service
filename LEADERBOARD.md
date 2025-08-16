## Leaderboard Points System

Our leaderboard uses an authoritative points calculation formula that rewards both finishing position and tournament difficulty. Each tournament result is worth a specific number of points based on field size, finishing position, and buy-in level.

### Authoritative Points Formula

**Formula:** `points = min(60, round( 3 * (sqrt(field_size) / sqrt(rank)) * (log10(buy_in_eur) + 1) + 2 ))`

**Where:**
- `field_size` = Number of players in the tournament
- `rank` = Final finishing position (1st, 2nd, 3rd, etc.)
- `buy_in_eur` = Tournament buy-in amount in euros
- `60` = Maximum points cap per tournament

### How the Formula Works

**1. Position Factor: `sqrt(field_size) / sqrt(rank)`**
- Rewards higher finishes more significantly
- Accounts for tournament field size (bigger fields = more points)
- Uses square root to prevent runaway scaling

**2. Buy-in Multiplier: `log10(buy_in_eur) + 1`**
- Higher buy-in tournaments award more points
- Logarithmic scaling prevents expensive tournaments from dominating
- Ensures all buy-in levels remain competitive

**3. Base Points: `+2`**
- Minimum point value for participating

**4. Scaling Factor: `×3`**
- Amplifies the calculated value to meaningful point ranges

**5. Point Cap: `min(60, ...)`**
- Prevents any single tournament from being worth too many points
- Maintains competitive balance across different tournament types

### Example Calculations

**Verified Test Cases:**

| Field Size | Position | Buy-in | Points | Calculation |
|------------|----------|---------|--------|-------------|
| 40 players | 1st place | €20 | **46 points** | `min(60, round(3 * (√40/√1) * (log₁₀(20)+1) + 2))` |
| 50 players | 2nd place | €30 | **39 points** | `min(60, round(3 * (√50/√2) * (log₁₀(30)+1) + 2))` |
| 80 players | 9th place | €50 | **26 points** | `min(60, round(3 * (√80/√9) * (log₁₀(50)+1) + 2))` |

**Additional Examples:**

| Scenario | Field | Position | Buy-in | Points |
|----------|-------|----------|---------|--------|
| Small tournament win | 12 players | 1st | €25 | ~27 points |
| Medium tournament final table | 30 players | 3rd | €35 | ~30 points |
| Large tournament deep run | 100 players | 15th | €50 | ~20 points |
| High-stakes tournament | 25 players | 5th | €100 | ~25 points |

### Total Leaderboard Score

**Your leaderboard ranking is determined by the sum of all your individual tournament points.**

Unlike complex formulas with multiple bonuses, this system is:
- **Transparent**: Every tournament result has a clear, calculable point value
- **Fair**: Rewards both skill (finishing position) and tournament difficulty
- **Balanced**: No single tournament type dominates the leaderboard
- **Consistent**: Same formula applies to all tournaments and players

### Leaderboard Statistics

Each player's profile displays:

- **User Information**: Full player details (name, username, contact info)
- **Total Tournaments**: Number of tournaments played
- **Total Spent**: Sum of all tournament buy-ins (in cents)
- **Total Won**: Sum of all prize money (in cents)
- **Net Profit/Loss**: Total won minus total spent
- **ITM%**: Percentage of tournaments finishing in the money (rounded to 2 decimals)
- **ROI%**: Return on investment percentage (rounded to 2 decimals)
- **Average Finish**: Mean finishing position across all tournaments (rounded to 2 decimals)
- **First Places**: Number of tournament victories
- **Final Tables**: Number of times finishing in top 9 positions
- **Total Points**: Sum of points from all tournament results

### Time Periods

Leaderboards can be filtered by:
- **All Time**: Complete tournament history
- **Last Year**: Rolling 365-day period
- **Last 6 Months**: Rolling 6-month period
- **Last 30 Days**: Rolling 30-day period
- **Last 7 Days**: Rolling 7-day period

This allows tracking both long-term consistency and recent performance trends.

### Point Calculation Timing

Points are automatically calculated when:
- Tournament results are entered through the system
- Results are updated or modified
- Historical data is backfilled (existing results are recalculated using the authoritative formula)

Each tournament result displays its individual point value, making the scoring system completely transparent to players.

---

*The authoritative formula ensures fair, consistent, and transparent scoring across all tournaments and players in the system.*