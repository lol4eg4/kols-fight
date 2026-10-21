-- KOLs and their published wallets. Hypotheses never live here.
CREATE TABLE registry.kol (
    id text PRIMARY KEY,
    name text NOT NULL,
    x_handle text,
    -- links
    kolexplorer text,
    solanatracker boolean NOT NULL DEFAULT false,
    kolscan boolean NOT NULL DEFAULT false,
    fomo text,
    pumpfun text,
    telegram text,
    -- reach
    x_followers bigint,
    telegram_subscribers bigint,
    pumpfun_followers bigint,
    reach_source text,
    reach_date date,
    formats text[] NOT NULL DEFAULT '{}',
    CHECK ((reach_source IS NULL) = (reach_date IS NULL))
);

CREATE INDEX kol_x_handle ON registry.kol (lower(x_handle));

-- One wallet belongs to one KOL: the address is the key.
CREATE TABLE registry.wallet (
    address text PRIMARY KEY,
    kol text NOT NULL REFERENCES registry.kol (id),
    main boolean NOT NULL,
    -- order of the other wallets
    pos integer NOT NULL,
    source text NOT NULL CHECK (source IN ('prototype', 'kolexplorer', 'network')),
    published_at timestamptz NOT NULL
);

CREATE UNIQUE INDEX wallet_one_main ON registry.wallet (kol) WHERE main;
CREATE INDEX wallet_kol ON registry.wallet (kol);
