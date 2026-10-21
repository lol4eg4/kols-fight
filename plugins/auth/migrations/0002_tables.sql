-- Staff, sessions and the login rate limit.

CREATE TABLE auth.staff (
    id            bigserial PRIMARY KEY,
    -- Lower-case, trimmed; unique.
    login         text NOT NULL UNIQUE,
    -- Argon2id PHC string. Never leaves this plugin.
    password_hash text NOT NULL,
    role          text NOT NULL CHECK (role IN ('staff', 'admin')),
    created_at    timestamptz NOT NULL,
    -- Set when access is revoked; the login stays reserved.
    revoked_at    timestamptz
);

CREATE TABLE auth.session (
    -- SHA-256 (hex) of the cookie token. The token itself is never stored.
    token_hash text PRIMARY KEY,
    staff_id   bigint NOT NULL REFERENCES auth.staff (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL
);

CREATE INDEX session_staff ON auth.session (staff_id);
CREATE INDEX session_expires ON auth.session (expires_at);

-- Failed logins per client address: 5 in 15 minutes lock the address for 15 minutes.
CREATE TABLE auth.login_limit (
    ip           text PRIMARY KEY,
    failures     integer NOT NULL,
    window_start timestamptz NOT NULL,
    locked_until timestamptz
);
