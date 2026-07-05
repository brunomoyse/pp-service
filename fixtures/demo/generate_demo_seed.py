#!/usr/bin/env python3
"""Generate the App Store review demo seed (demo_seed.sql).

Builds a realistic dataset for the pocketpair.app pilot environment:
two clubs, a cast of Belgian players, ~10 weeks of finished tournaments
with entries/results/payouts, upcoming events the reviewer can register
for, achievements, attendance streaks and a season.

Deterministic: fixed RNG seed + uuid5 ids, so re-running produces the
same SQL. Dates are anchored on ANCHOR (a Saturday); regenerate and
re-apply whenever the demo needs to look "fresh" again.

Usage:  python3 generate_demo_seed.py > demo_seed.sql
Apply:  psql "$DATABASE_URL" -f fixtures/00_cleanup.sql -f fixtures/demo/demo_seed.sql
"""

import json
import random
import uuid
from datetime import datetime, timedelta, timezone

rng = random.Random(20260704)
NS = uuid.UUID("f0c7cc4e-2f2b-4e37-9a2e-0d5b7a1c9e11")


def uid(*parts: object) -> str:
    return str(uuid.uuid5(NS, ":".join(str(p) for p in parts)))


ANCHOR = datetime(2026, 7, 4, tzinfo=timezone.utc)  # regenerate when stale

# Both demo logins share this password: PocketPair2026
HASH_DEMO = "$2b$12$4.9dd0XFONNttsTDb4YrIOHPgClE4n1vpZc9liC0YigLYmI7uckbO"
# Pre-existing owner admin (password: admin), kept from fixtures/02_users.sql
HASH_ADMIN = "$2b$12$YkLUdU.KpnCZ78RF3eOMxuDK3DahuGBxf9Q.fqY1oqAZLQovqdjA6"

CLUB_LIEGE = uid("club", "liege")
CLUB_ANTWERP = uid("club", "antwerp")

REVIEWER = uid("user", "reviewer")
MANAGER = uid("user", "manager")
ADMIN = uid("user", "admin")

# (key, first, last, username, locale)
PLAYERS = [
    ("maxime", "Maxime", "Dupont", "maxthegrinder", "fr"),
    ("julien", "Julien", "Lambert", "jlamb_poker", "fr"),
    ("nicolas", "Nicolas", "Peeters", "nico_peeters", "nl"),
    ("thomas", "Thomas", "Janssens", "tommy_jans", "nl"),
    ("lucas", "Lucas", "Dubois", "lucky_dubois", "fr"),
    ("antoine", "Antoine", "Lejeune", "antoine_lj", "fr"),
    ("hugo", "Hugo", "Claes", "hugoclaes", "nl"),
    ("sofie", "Sofie", "Vermeulen", "sofie_v", "nl"),
    ("emma", "Emma", "Van Damme", "emma_vd", "nl"),
    ("lea", "Léa", "François", "lea_frcs", "fr"),
    ("camille", "Camille", "Renard", "cam_renard", "fr"),
    ("sarah", "Sarah", "Goossens", "sgoossens", "nl"),
    ("manon", "Manon", "Gérard", "manon_g", "fr"),
    ("julie", "Julie", "Mertens", "julie_m", "nl"),
    ("kevin", "Kevin", "De Smet", "kdesmet", "nl"),
    ("dries", "Dries", "Wouters", "dries_w", "nl"),
    ("bart", "Bart", "Willems", "bartwillems", "nl"),
    ("olivier", "Olivier", "Simon", "oli_simon", "fr"),
    ("mathieu", "Mathieu", "Collard", "mcollard", "fr"),
    ("pierre", "Pierre", "Bodart", "pbodart", "fr"),
    ("yannick", "Yannick", "Dumont", "yannick_d", "fr"),
    ("florian", "Florian", "Petit", "flo_petit", "fr"),
]

OUT: list[str] = []


def emit(sql: str) -> None:
    OUT.append(sql)


def q(s: str) -> str:
    return "'" + s.replace("'", "''") + "'"


def ts(dt: datetime) -> str:
    return "'" + dt.strftime("%Y-%m-%d %H:%M:%S+00") + "'"


# ---------------------------------------------------------------- users
emit("-- === Users ===")
rows = [
    f"({q(REVIEWER)}, 'reviewer@pocketpair.app', 'alexmartin', 'Alex', 'Martin', 'player', 'en', {q(HASH_DEMO)}, NOW() - interval '1 day')",
    f"({q(MANAGER)}, 'manager@pocketpair.app', 'marc_ldc', 'Marc', 'Delvaux', 'manager', 'fr', {q(HASH_DEMO)}, NOW() - interval '2 hours')",
    f"({q(ADMIN)}, 'moyse94@gmail.com', 'super_admin', 'Admin', 'Global', 'admin', 'en', {q(HASH_ADMIN)}, NOW())",
]
for key, first, last, username, locale in PLAYERS:
    seen = rng.randint(1, 96)
    rows.append(
        f"({q(uid('user', key))}, '{key}.{last.lower().replace(' ', '')}@demo.pocketpair.app', "
        f"{q(username)}, {q(first)}, {q(last)}, 'player', '{locale}', NULL, NOW() - interval '{seen} hours')"
    )
emit(
    "INSERT INTO users (id, email, username, first_name, last_name, role, locale, password_hash, last_seen_at) VALUES\n"
    + ",\n".join(rows) + ";"
)

# ---------------------------------------------------------------- clubs
emit("\n-- === Clubs (insert re-fires the default template triggers) ===")
emit(
    "INSERT INTO clubs (id, name, city, country, address, postal_code, plan) VALUES\n"
    f"({q(CLUB_LIEGE)}, 'Soumagne Poker Club', 'Soumagne', 'BE', 'Rue du Centre 12', '4630', 'club'),\n"
    f"({q(CLUB_ANTWERP)}, 'Antwerp Card Room', 'Antwerpen', 'BE', 'Lange Koepoortstraat 47', '2000', 'club');"  # 'free' clubs are hidden from player discovery
)
emit(
    "INSERT INTO club_managers (club_id, user_id) VALUES\n"
    f"({q(CLUB_LIEGE)}, {q(MANAGER)}), ({q(CLUB_ANTWERP)}, {q(MANAGER)});"
)
emit(
    "INSERT INTO club_tables (id, club_id, table_number, max_seats, is_default) VALUES\n"
    + ",\n".join(
        f"({q(uid('table', c, n))}, {q(c)}, {n}, 9, true)"
        for c, count in ((CLUB_LIEGE, 4), (CLUB_ANTWERP, 2))
        for n in range(1, count + 1)
    )
    + ";"
)

# club_player roster rows (club-scoped identity used by regs/entries/results)
emit("\n-- === Club rosters ===")
ALL_USERS = [("reviewer", REVIEWER, "Alex Martin"), ("manager", MANAGER, "Marc Delvaux")] + [
    (key, uid("user", key), f"{first} {last}") for key, first, last, _, _ in PLAYERS
]
cp_rows = []
for club in (CLUB_LIEGE, CLUB_ANTWERP):
    for key, user_id, display in ALL_USERS:
        cp_rows.append(f"({q(uid('cp', club, key))}, {q(club)}, {q(display)}, {q(user_id)})")
emit("INSERT INTO club_player (id, club_id, display_name, app_user_id) VALUES\n" + ",\n".join(cp_rows) + ";")


def cp(club: str, key: str) -> str:
    return uid("cp", club, key)


# ---------------------------------------------------------- tournaments
LEVELS = [  # (sb, bb, ante, minutes, is_break)
    (25, 50, 0, 20, False), (50, 100, 0, 20, False), (75, 150, 0, 20, False),
    (100, 200, 200, 20, False), (0, 0, 0, 15, True), (150, 300, 300, 20, False),
    (200, 400, 400, 20, False), (300, 600, 600, 20, False), (400, 800, 800, 20, False),
    (0, 0, 0, 15, True), (500, 1000, 1000, 20, False), (600, 1200, 1200, 20, False),
    (800, 1600, 1600, 20, False), (1000, 2000, 2000, 20, False),
]

PAYOUT_PCT = {  # by field size, mirrors the club default templates
    (3, 10): [70, 30],
    (11, 20): [50, 30, 20],
    (21, 30): [37, 25, 15, 12, 11],
}


def payout_split(pool: int, entrants: int) -> list[int]:
    for (lo, hi), pcts in PAYOUT_PCT.items():
        if lo <= entrants <= hi:
            prizes = [int(round(pool * p / 100 / 500)) * 500 for p in pcts]
            prizes[0] += pool - sum(prizes)  # winner absorbs rounding
            return prizes
    return [pool]


class T:
    def __init__(self, key, club, name, desc, start, buy_in, rake, stack,
                 status, bounty=0, seat_cap=None, edition=None):
        self.key, self.club, self.name, self.desc = key, club, name, desc
        self.start, self.buy_in, self.rake, self.stack = start, buy_in, rake, stack
        self.status, self.bounty, self.seat_cap = status, bounty, seat_cap
        self.id = uid("tournament", key)
        self.participants: list[tuple[str, str]] = []  # (user_key, user_id) in finish order
        self.rebuys: list[str] = []


def fri(weeks_back: int) -> datetime:  # anchor 2026-07-04 is a Saturday
    return ANCHOR - timedelta(days=1 + 7 * weeks_back) + timedelta(hours=18)  # 20:00 CEST


def sun(weeks_back: int) -> datetime:
    return ANCHOR - timedelta(days=6 + 7 * (weeks_back - 1)) + timedelta(hours=13)  # 15:00 CEST


tournaments: list[T] = []

# Finished — Friday Night Deepstack #14..#24 (11 weeks of history, ~2.5 months)
for i, wb in enumerate(range(11, 0, -1)):
    tournaments.append(T(
        f"deepstack{14 + i}", CLUB_LIEGE, f"Friday Night Deepstack #{14 + i}",
        "Weekly deepstack — 25k stack, 20-minute levels, late reg until level 6.",
        fri(wb), 5000, 500, 25000, "finished",
    ))

# Finished — Sunday Bounty #3..#8 (every other Sunday, spanning ~11 weeks)
for i, wb in enumerate((11, 9, 7, 5, 3, 1)):
    tournaments.append(T(
        f"bounty{3 + i}", CLUB_LIEGE, f"Sunday Bounty #{3 + i}",
        "€10 fixed bounty per knockout. Rebuys allowed for the first four levels.",
        sun(wb), 4000, 500, 20000, "finished", bounty=1000,
    ))

# Finished — Monthly Main Event #1/#2 (last Saturday of May / June)
tournaments.append(T(
    "main1", CLUB_LIEGE, "Monthly Main Event #1",
    "Our flagship monthly freezeout. 30k stack, 30-minute levels, top 5 paid.",
    ANCHOR - timedelta(days=35) + timedelta(hours=17), 15000, 1000, 30000, "finished",
))
tournaments.append(T(
    "main2", CLUB_LIEGE, "Monthly Main Event #2",
    "Our flagship monthly freezeout. 30k stack, 30-minute levels, top 5 paid.",
    ANCHOR - timedelta(days=7) + timedelta(hours=17), 15000, 1000, 30000, "finished",
))

# Finished — one Antwerp event the reviewer travelled to (cross-club passport)
tournaments.append(T(
    "antwerp_freeze", CLUB_ANTWERP, "Sunday Freezeout",
    "Classic freezeout, one bullet, 20k stack.",
    ANCHOR - timedelta(days=13) + timedelta(hours=13), 4000, 400, 20000, "finished",
))

# Upcoming
upcoming = [
    T("deepstack25", CLUB_LIEGE, "Friday Night Deepstack #25",
      "Weekly deepstack — 25k stack, 20-minute levels, late reg until level 6.",
      ANCHOR + timedelta(days=6, hours=18), 5000, 500, 25000, "registration_open"),
    T("bounty9", CLUB_LIEGE, "Sunday Bounty #9",
      "€10 fixed bounty per knockout. Rebuys allowed for the first four levels.",
      ANCHOR + timedelta(days=8, hours=13), 4000, 500, 20000, "registration_open", bounty=1000),
    T("main3", CLUB_LIEGE, "Monthly Main Event #3",
      "Our flagship monthly freezeout. 30k stack, 30-minute levels, top 5 paid.",
      ANCHOR + timedelta(days=14, hours=17), 15000, 1000, 30000, "registration_open", seat_cap=40),
    T("antwerp_turbo", CLUB_ANTWERP, "Midweek Turbo",
      "Fast structure, 12-minute levels — done before midnight.",
      ANCHOR + timedelta(days=5, hours=17, minutes=30), 3000, 300, 15000, "registration_open"),
]
tournaments.extend(upcoming)

# --------------------------------------------------- fields & reviewer script
# Reviewer's arc: 6 events over ~9 weeks (3 within the last 30 days, 3 older) so the
# 30-day and 1-year stat views clearly differ. 3 cashes incl. one win; ITM 50%.
# The win sits ~5 weeks back, so it shows up in the 1-year view but not the 30-day one.
REVIEWER_SCRIPT = {  # tournament key -> final position
    "deepstack16": 12,    # ~9 weeks back
    "deepstack18": 9,     # ~7 weeks back
    "deepstack20": 1,     # ~5 weeks back, the win (outside the 30-day window)
    "deepstack23": 3,     # ~2 weeks back, cash
    "antwerp_freeze": 8,  # ~13 days back
    "deepstack24": 2,     # ~1 week back, cash
}

player_keys = [k for k, *_ in PLAYERS]
for t in tournaments:
    if t.status != "finished":
        continue
    n = rng.randint(16, 22)
    field = rng.sample(player_keys, n)
    if rng.random() < 0.4 and "manager" not in field:  # Marc plays sometimes
        field[rng.randrange(n)] = "manager"
    if t.key in REVIEWER_SCRIPT:
        pos = REVIEWER_SCRIPT[t.key]
        field = [k for k in field if k != "reviewer"]
        field.insert(pos - 1, "reviewer")
    t.participants = [(k, REVIEWER if k == "reviewer" else (MANAGER if k == "manager" else uid("user", k))) for k in field]
    if t.bounty:
        t.rebuys = [k for k, _ in t.participants if k != "reviewer" and rng.random() < 0.2]

# Upcoming registration lists (reviewer pre-registered for Sunday Bounty #9)
upcoming_regs = {
    "deepstack25": rng.sample(player_keys, 9),
    "bounty9": ["reviewer"] + rng.sample(player_keys, 7),
    "main3": rng.sample(player_keys, 14),
    "antwerp_turbo": rng.sample(player_keys, 5),
}

# ------------------------------------------------------------- emit SQL
emit("\n-- === Tournaments ===")
t_rows = []
for t in tournaments:
    end = ts(t.start + timedelta(hours=5)) if t.status == "finished" else "NULL"
    cap = t.seat_cap if t.seat_cap else "NULL"
    bounty_type = "'fixed'" if t.bounty else "'none'"
    t_rows.append(
        f"({q(t.id)}, {q(t.club)}, {q(t.name)}, {q(t.desc)}, {ts(t.start)}, {end}, "
        f"{t.buy_in}, {t.rake}, {cap}, {t.stack}, '{t.status}', {bounty_type}, {t.bounty}, 6)"
    )
emit(
    "INSERT INTO tournaments (id, club_id, name, description, start_time, end_time, "
    "buy_in_cents, rake_cents, seat_cap, starting_stack, live_status, bounty_type, "
    "bounty_amount_cents, late_registration_level) VALUES\n" + ",\n".join(t_rows) + ";"
)

emit("\n-- === Blind structures ===")
s_rows = []
for t in tournaments:
    for lvl, (sb, bb, ante, mins, brk) in enumerate(LEVELS, start=1):
        brk_mins = mins if brk else "NULL"
        s_rows.append(
            f"({q(t.id)}, {lvl}, {sb}, {bb}, {ante}, {mins}, {str(brk).lower()}, {brk_mins})"
        )
emit(
    "INSERT INTO tournament_structures (tournament_id, level_number, small_blind, big_blind, "
    "ante, duration_minutes, is_break, break_duration_minutes) VALUES\n" + ",\n".join(s_rows) + ";"
)

emit("\n-- === Registrations, entries, check-ins ===")
reg_rows, entry_rows, checkin_rows = [], [], []
for t in tournaments:
    if t.status == "finished":
        for i, (key, user_id) in enumerate(t.participants):
            reg_time = t.start - timedelta(days=rng.randint(1, 5), minutes=rng.randint(0, 600))
            reg_rows.append(
                f"({q(t.id)}, {q(user_id)}, {q(cp(t.club, key))}, {ts(reg_time)}, 'busted')"
            )
            entry_rows.append(
                f"({q(t.id)}, {q(user_id)}, {q(cp(t.club, key))}, 'initial', {t.buy_in}, {t.stack}, 'cash', {q(MANAGER)}, {ts(t.start)})"
            )
            checkin_rows.append(
                f"({q(user_id)}, {q(t.id)}, {q(t.club)}, {ts(t.start - timedelta(minutes=rng.randint(5, 40)))})"
            )
        for key in t.rebuys:
            user_id = MANAGER if key == "manager" else uid("user", key)
            entry_rows.append(
                f"({q(t.id)}, {q(user_id)}, {q(cp(t.club, key))}, 'rebuy', {t.buy_in}, {t.stack}, 'cash', {q(MANAGER)}, {ts(t.start + timedelta(minutes=rng.randint(30, 90)))})"
            )
    else:
        for key in upcoming_regs[t.key]:
            user_id = REVIEWER if key == "reviewer" else uid("user", key)
            reg_time = ANCHOR - timedelta(hours=rng.randint(2, 72))
            reg_rows.append(
                f"({q(t.id)}, {q(user_id)}, {q(cp(t.club, key))}, {ts(reg_time)}, 'registered')"
            )
emit(
    "INSERT INTO tournament_registrations (tournament_id, user_id, club_player_id, registration_time, status) VALUES\n"
    + ",\n".join(reg_rows) + ";"
)
emit(
    "INSERT INTO tournament_entries (tournament_id, user_id, club_player_id, entry_type, "
    "amount_cents, chips_received, payment_method, recorded_by, created_at) VALUES\n"
    + ",\n".join(entry_rows) + ";"
)
emit(
    "INSERT INTO check_in (app_user_id, tournament_id, club_id, checked_in_at) VALUES\n"
    + ",\n".join(checkin_rows) + ";"
)

emit("\n-- === Results & payouts ===")
res_rows, payout_rows = [], []
for t in tournaments:
    if t.status != "finished":
        continue
    n = len(t.participants)
    entries_total = (n + len(t.rebuys)) * t.buy_in
    pool = entries_total - n * t.rake - (n + len(t.rebuys)) * t.bounty
    prizes = payout_split(pool, n)
    for pos, (key, user_id) in enumerate(t.participants, start=1):
        prize = prizes[pos - 1] if pos <= len(prizes) else 0
        points = max(n - pos + 1, 0)
        res_rows.append(
            f"({q(t.id)}, {q(user_id)}, {q(cp(t.club, key))}, {pos}, {prize}, {points})"
        )
    positions = json.dumps(
        [{"position": i + 1, "amount_cents": p} for i, p in enumerate(prizes)]
    )
    payout_rows.append(f"({q(t.id)}, {n}, {pool}, {q(positions)}::jsonb)")
emit(
    "INSERT INTO tournament_results (tournament_id, user_id, club_player_id, final_position, prize_cents, points) VALUES\n"
    + ",\n".join(res_rows) + ";"
)
emit(
    "INSERT INTO tournament_payouts (tournament_id, player_count, total_prize_pool, payout_positions) VALUES\n"
    + ",\n".join(payout_rows)
    + "\nON CONFLICT (tournament_id) DO UPDATE SET player_count = EXCLUDED.player_count, "
    "total_prize_pool = EXCLUDED.total_prize_pool, payout_positions = EXCLUDED.payout_positions;"
)

emit("\n-- === Achievements (derived from the seeded results, so they stay consistent) ===")
stats: dict[str, dict] = {}
for t in tournaments:
    if t.status != "finished":
        continue
    n = len(t.participants)
    prizes = payout_split((n + len(t.rebuys)) * t.buy_in - n * t.rake - (n + len(t.rebuys)) * t.bounty, n)
    for pos, (key, user_id) in enumerate(t.participants, start=1):
        s = stats.setdefault(key, {"user_id": user_id, "played": [], "cashes": [], "wins": [], "clubs": set(), "winnings": 0})
        s["played"].append((t.start, t.id))
        s["clubs"].add(t.club)
        if pos <= len(prizes):
            s["cashes"].append((t.start, t.id))
            s["winnings"] += prizes[pos - 1]
        if pos == 1:
            s["wins"].append((t.start, t.id))

ach_rows = []
def ach(user_id, code, when, tid, progress):
    tid_sql = f"{q(tid)}::uuid" if tid else "NULL::uuid"
    ach_rows.append(
        f"SELECT {q(user_id)}::uuid, id, {ts(when)}::timestamptz, {tid_sql}, {progress} FROM achievements WHERE code = '{code}'"
    )

for key, s in stats.items():
    s["played"].sort(); s["cashes"].sort(); s["wins"].sort()
    first_start, first_tid = s["played"][0]
    ach(s["user_id"], "first_registration", first_start, first_tid, 1)
    if s["cashes"]:
        ach(s["user_id"], "first_cash", s["cashes"][0][0], s["cashes"][0][1], 1)
    if s["wins"]:
        ach(s["user_id"], "first_win", s["wins"][0][0], s["wins"][0][1], 1)
    if len(s["played"]) >= 5:
        ach(s["user_id"], "tournaments_5", s["played"][4][0], s["played"][4][1], len(s["played"]))
    if len(s["clubs"]) >= 2:
        ach(s["user_id"], "clubs_2", s["played"][-1][0], s["played"][-1][1], 2)
    itm = round(100 * len(s["cashes"]) / len(s["played"]))
    if itm >= 50 and len(s["played"]) >= 4:
        ach(s["user_id"], "itm_rate_50", s["cashes"][-1][0], s["cashes"][-1][1], itm)
# Reviewer extra: 5-week attendance streak unlocked during the finale week
ach(REVIEWER, "streak_play_5", stats["reviewer"]["played"][-1][0], stats["reviewer"]["played"][-1][1], 5)

emit(
    "INSERT INTO player_achievements (user_id, achievement_id, unlocked_at, tournament_id, progress)\n"
    + "\nUNION ALL\n".join(ach_rows) + ";"
)

emit("\n-- === Attendance streaks ===")
streak_rows = []
for key, s in stats.items():
    played_n = len(s["played"])
    current = min(played_n, rng.randint(0, 5))
    longest = max(current, min(played_n, rng.randint(1, 7)))
    if key == "reviewer":
        current, longest = 4, 5
    streak_rows.append(
        f"({q(s['user_id'])}, {current}, {longest}, {ts(s['played'][-1][0])})"
    )
emit(
    "INSERT INTO attendance_streak (app_user_id, current_streak, longest_streak, last_check_in_at) VALUES\n"
    + ",\n".join(streak_rows) + ";"
)

emit("\n-- === Season & league ===")
season_id = uid("season", "summer2026")
emit(
    "INSERT INTO season (id, club_id, name, starts_at, ends_at) VALUES\n"
    f"({q(season_id)}, {q(CLUB_LIEGE)}, 'Summer Series 2026', "
    f"{ts(datetime(2026, 6, 1, tzinfo=timezone.utc))}, {ts(datetime(2026, 8, 31, 22, tzinfo=timezone.utc))});"
)
pass_keys = ["reviewer"] + [k for k, *_ in PLAYERS[:10]]
emit(
    "INSERT INTO season_pass (season_id, app_user_id) VALUES\n"
    + ",\n".join(f"({q(season_id)}, {q(REVIEWER if k == 'reviewer' else uid('user', k))})" for k in pass_keys)
    + ";"
)
config_id = uid("lbconfig", "summer2026")
# Default scoring formula, mirrors infra::scoring::ScoringFormula::default()
FORMULA = '{"base_points": 2.0, "field_multiplier": 3.0, "buyin_multiplier": 1.0, "min_players": 1, "cap": 60}'
emit(
    "INSERT INTO leaderboard_configs (id, club_id, name, formula_params, membership_mode, period_start, period_end, is_default) VALUES\n"
    f"({q(config_id)}, {q(CLUB_LIEGE)}, 'Summer Series 2026', {q(FORMULA)}::jsonb, 'all_in_period', "
    f"{ts(datetime(2026, 6, 1, tzinfo=timezone.utc))}, {ts(datetime(2026, 8, 31, 22, tzinfo=timezone.utc))}, true);"
)
emit(
    f"UPDATE tournaments SET leaderboard_config_id = {q(config_id)} "
    f"WHERE club_id = {q(CLUB_LIEGE)} AND start_time >= {ts(datetime(2026, 6, 1, tzinfo=timezone.utc))};"
)

print("-- App Store review demo seed — GENERATED by generate_demo_seed.py, do not edit by hand.")
print(f"-- Anchor date: {ANCHOR.date()}. Regenerate + re-apply when the data looks stale.")
print("-- Apply after fixtures/00_cleanup.sql. Logins: reviewer@pocketpair.app /")
print("-- manager@pocketpair.app, password PocketPair2026 (admin unchanged).")
print("BEGIN;")
print("\n".join(OUT))
print("COMMIT;")
