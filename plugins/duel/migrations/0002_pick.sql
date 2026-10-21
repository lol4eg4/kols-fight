-- The pair of the day, stored the first time it is asked for so every visitor sees the same
-- pair even if the day is ranked again.
CREATE TABLE duel.pick (
    on_date   date PRIMARY KEY,
    kol_a     text NOT NULL,
    kol_b     text NOT NULL,
    picked_at timestamptz NOT NULL DEFAULT now(),
    CHECK (kol_a <> kol_b)
);
