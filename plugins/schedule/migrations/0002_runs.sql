-- One row per collection run (scheduled 06:00, «Run now», or start-up recovery) and one per
-- step of the run. A run left `running` by a restart is marked `interrupted` at the next start.
CREATE TABLE schedule.run (
    id          bigserial PRIMARY KEY,
    date        date NOT NULL,
    trigger     text NOT NULL CHECK (trigger IN ('scheduled', 'manual', 'recovery')),
    person_id   bigint,
    started_at  timestamptz NOT NULL,
    finished_at timestamptz,
    status      text NOT NULL CHECK (status IN ('running', 'ok', 'partial', 'failed', 'interrupted')),
    error       text
);
CREATE INDEX run_started ON schedule.run (started_at DESC);
CREATE INDEX run_running ON schedule.run (status) WHERE status = 'running';

-- Steps in order: open, event, kolexplorer, solanatracker, close, rank.
-- `rows` is the source's row count (KOL Explorer top rows, Solana Tracker list rows).
CREATE TABLE schedule.step (
    run_id      bigint NOT NULL REFERENCES schedule.run (id) ON DELETE CASCADE,
    step        text NOT NULL,
    seq         integer NOT NULL,
    status      text NOT NULL CHECK (status IN ('ok', 'partial', 'failed')),
    started_at  timestamptz NOT NULL,
    finished_at timestamptz NOT NULL,
    rows        integer,
    error       text,
    note        text,
    PRIMARY KEY (run_id, step)
);
