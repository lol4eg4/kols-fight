-- One snapshot per date: members (who is on the board and why) and one metric row per KOL.
CREATE TABLE snapshots.snapshot (
    date date PRIMARY KEY,
    status text NOT NULL CHECK (status IN ('open', 'closed')),
    opened_at timestamptz NOT NULL,
    closed_at timestamptz
);

CREATE TABLE snapshots.member (
    date date NOT NULL REFERENCES snapshots.snapshot (date),
    kol text NOT NULL,
    reason text NOT NULL CHECK (reason IN ('top100', 'network')),
    PRIMARY KEY (date, kol, reason)
);

-- A missing metric is NULL, never zero.
CREATE TABLE snapshots.metric (
    date date NOT NULL REFERENCES snapshots.snapshot (date),
    kol text NOT NULL,
    source text NOT NULL CHECK (source IN ('kolexplorer', 'solanatracker')),
    fetched_at timestamptz NOT NULL,
    d1 double precision,
    d7 double precision,
    d30 double precision,
    wr7 double precision,
    wr30 double precision,
    wr_all double precision,
    avg_pos double precision,
    hold_s double precision,
    last_min_at_fetch bigint,
    traded_24h boolean,
    unreal double precision,
    total double precision,
    vol30 double precision,
    days30 integer,
    wins integer,
    losses integer,
    pnl_rank integer,
    in_top_today boolean NOT NULL DEFAULT false,
    PRIMARY KEY (date, kol)
);

CREATE INDEX metric_kol_date ON snapshots.metric (kol, date);
