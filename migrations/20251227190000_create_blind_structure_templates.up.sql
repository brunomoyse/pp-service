-- Create blind structure templates table
CREATE TABLE blind_structure_templates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    levels JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

-- Seed default templates

-- Quick Tournament (1h) - 6 levels × 10 min, no breaks
INSERT INTO blind_structure_templates (name, description, levels) VALUES (
    'Quick Tournament (1h)',
    'Fast-paced tournament with 10-minute levels. Perfect for casual games.',
    '[
        {"levelNumber": 1, "smallBlind": 25, "bigBlind": 50, "ante": 0, "durationMinutes": 10, "isBreak": false},
        {"levelNumber": 2, "smallBlind": 50, "bigBlind": 100, "ante": 0, "durationMinutes": 10, "isBreak": false},
        {"levelNumber": 3, "smallBlind": 100, "bigBlind": 200, "ante": 0, "durationMinutes": 10, "isBreak": false},
        {"levelNumber": 4, "smallBlind": 200, "bigBlind": 400, "ante": 0, "durationMinutes": 10, "isBreak": false},
        {"levelNumber": 5, "smallBlind": 400, "bigBlind": 800, "ante": 0, "durationMinutes": 10, "isBreak": false},
        {"levelNumber": 6, "smallBlind": 600, "bigBlind": 1200, "ante": 0, "durationMinutes": 10, "isBreak": false}
    ]'::jsonb
);

-- Standard Tournament (2h) - 8 levels × 15 min + 1 break, antes from level 5
INSERT INTO blind_structure_templates (name, description, levels) VALUES (
    'Standard Tournament (2h)',
    'Classic structure with 15-minute levels and one break. Balanced for most home games.',
    '[
        {"levelNumber": 1, "smallBlind": 25, "bigBlind": 50, "ante": 0, "durationMinutes": 15, "isBreak": false},
        {"levelNumber": 2, "smallBlind": 50, "bigBlind": 100, "ante": 0, "durationMinutes": 15, "isBreak": false},
        {"levelNumber": 3, "smallBlind": 75, "bigBlind": 150, "ante": 0, "durationMinutes": 15, "isBreak": false},
        {"levelNumber": 4, "smallBlind": 100, "bigBlind": 200, "ante": 0, "durationMinutes": 15, "isBreak": false},
        {"levelNumber": 5, "smallBlind": 0, "bigBlind": 0, "ante": 0, "durationMinutes": 15, "isBreak": true, "breakDurationMinutes": 15},
        {"levelNumber": 6, "smallBlind": 150, "bigBlind": 300, "ante": 25, "durationMinutes": 15, "isBreak": false},
        {"levelNumber": 7, "smallBlind": 200, "bigBlind": 400, "ante": 50, "durationMinutes": 15, "isBreak": false},
        {"levelNumber": 8, "smallBlind": 300, "bigBlind": 600, "ante": 75, "durationMinutes": 15, "isBreak": false},
        {"levelNumber": 9, "smallBlind": 400, "bigBlind": 800, "ante": 100, "durationMinutes": 15, "isBreak": false}
    ]'::jsonb
);

-- Deep Stack (3h+) - 12 levels × 20 min + 2 breaks, antes from level 5
INSERT INTO blind_structure_templates (name, description, levels) VALUES (
    'Deep Stack (3h+)',
    'Longer levels for skilled play with gradual blind progression. Ideal for serious tournaments.',
    '[
        {"levelNumber": 1, "smallBlind": 25, "bigBlind": 50, "ante": 0, "durationMinutes": 20, "isBreak": false},
        {"levelNumber": 2, "smallBlind": 50, "bigBlind": 100, "ante": 0, "durationMinutes": 20, "isBreak": false},
        {"levelNumber": 3, "smallBlind": 75, "bigBlind": 150, "ante": 0, "durationMinutes": 20, "isBreak": false},
        {"levelNumber": 4, "smallBlind": 100, "bigBlind": 200, "ante": 0, "durationMinutes": 20, "isBreak": false},
        {"levelNumber": 5, "smallBlind": 0, "bigBlind": 0, "ante": 0, "durationMinutes": 15, "isBreak": true, "breakDurationMinutes": 15},
        {"levelNumber": 6, "smallBlind": 150, "bigBlind": 300, "ante": 25, "durationMinutes": 20, "isBreak": false},
        {"levelNumber": 7, "smallBlind": 200, "bigBlind": 400, "ante": 50, "durationMinutes": 20, "isBreak": false},
        {"levelNumber": 8, "smallBlind": 300, "bigBlind": 600, "ante": 75, "durationMinutes": 20, "isBreak": false},
        {"levelNumber": 9, "smallBlind": 0, "bigBlind": 0, "ante": 0, "durationMinutes": 15, "isBreak": true, "breakDurationMinutes": 15},
        {"levelNumber": 10, "smallBlind": 400, "bigBlind": 800, "ante": 100, "durationMinutes": 20, "isBreak": false},
        {"levelNumber": 11, "smallBlind": 600, "bigBlind": 1200, "ante": 150, "durationMinutes": 20, "isBreak": false},
        {"levelNumber": 12, "smallBlind": 800, "bigBlind": 1600, "ante": 200, "durationMinutes": 20, "isBreak": false}
    ]'::jsonb
);

-- Create index for efficient lookups
CREATE INDEX idx_blind_structure_templates_name ON blind_structure_templates(name);
