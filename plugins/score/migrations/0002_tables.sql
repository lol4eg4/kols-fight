-- Stored ranking of a closed snapshot.
CREATE TABLE score.ranking (
    date date PRIMARY KEY,
    ranked_at timestamptz NOT NULL
);

-- One KOL's result: parts and badges as JSON (api_score::Part, api_score::Badge).
CREATE TABLE score.result (
    date date NOT NULL REFERENCES score.ranking (date) ON DELETE CASCADE,
    kol text NOT NULL,
    rank integer NOT NULL,
    score smallint NOT NULL CHECK (score BETWEEN 0 AND 100),
    tier text NOT NULL,
    parts jsonb NOT NULL,
    badges jsonb NOT NULL,
    PRIMARY KEY (date, kol),
    UNIQUE (date, rank)
);
