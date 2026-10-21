-- Profile pages read in the last 24 hours: a re-run the same day reuses them.
-- Only successful reads are stored, so a failed profile is tried again.
CREATE TABLE kolexplorer.profile_cache (
    slug       text PRIMARY KEY,
    fetched_at timestamptz NOT NULL,
    avg_pos    double precision,
    hold_s     double precision
);

-- «Wallet - X handle» list of the last successful leaderboard read (all rows, 150 today).
CREATE TABLE kolexplorer.board_row (
    wallet  text PRIMARY KEY,
    rank    integer NOT NULL,
    slug    text NOT NULL,
    name    text NOT NULL,
    x_handle text,
    read_at timestamptz NOT NULL
);
CREATE INDEX board_row_handle ON kolexplorer.board_row (lower(x_handle));
