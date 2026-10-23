-- yAPPA control plane, schema 1. Migrations are additive only: an older server binary must
-- keep running against a newer database during an update (vision, section 4).

CREATE TABLE users (
    id               TEXT PRIMARY KEY,           -- LiveKit identity
    display_name     TEXT NOT NULL,
    role             TEXT NOT NULL CHECK (role IN ('member', 'admin')),
    created_at       TEXT NOT NULL,              -- RFC 3339
    app_version      TEXT,                       -- from the last login / token request
    protocol_version INTEGER,
    platform         TEXT,
    last_seen        TEXT
);

-- Bearer tokens are random; only their SHA-256 is stored.
CREATE TABLE sessions (
    token_hash TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TEXT NOT NULL
);

CREATE TABLE invites (
    code       TEXT PRIMARY KEY,
    role       TEXT NOT NULL CHECK (role IN ('member', 'admin')),
    created_by TEXT NOT NULL,                    -- user id, or 'cli' for the bootstrap invite
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    used_by    TEXT REFERENCES users (id) ON DELETE SET NULL
);

CREATE TABLE channels (
    id        TEXT PRIMARY KEY,                  -- = LiveKit room name
    name      TEXT NOT NULL,
    position  INTEGER NOT NULL,
    server_id TEXT NOT NULL
);

-- Small key/value store: min_version, killed_versions (JSON array).
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE events (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    time       TEXT NOT NULL,
    kind       TEXT NOT NULL,
    user_id    TEXT,
    channel_id TEXT,
    message    TEXT NOT NULL
);

CREATE INDEX events_time ON events (time);

INSERT INTO channels (id, name, position, server_id) VALUES ('lobby', 'Lobby', 0, 'main');
