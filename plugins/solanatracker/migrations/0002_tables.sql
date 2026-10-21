-- The day's KOL list (30-day KOL leaderboard), one per date; a re-run replaces its date.
CREATE TABLE solanatracker.kol_list (
    list_date  date PRIMARY KEY,
    fetched_at timestamptz NOT NULL
);

CREATE TABLE solanatracker.listed_kol (
    list_date    date NOT NULL REFERENCES solanatracker.kol_list (list_date) ON DELETE CASCADE,
    position     integer NOT NULL,
    wallet       text NOT NULL,
    name         text,
    x_handle     text,
    fomo_handle  text,
    PRIMARY KEY (list_date, position)
);
CREATE INDEX listed_kol_handle ON solanatracker.listed_kol (lower(x_handle));
