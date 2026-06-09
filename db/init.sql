-- Learning Management System — database schema
-- This script runs automatically on first boot of the Postgres container
-- (postgres-entrypoint executes *.sql in /docker-entrypoint-initdb.d once,
-- when the data directory is empty).
--
-- It creates every table, the role enum, the updated_at triggers and the
-- server-side session store. The seed admin user is NOT created here on
-- purpose: credentials must never live in source. The application bootstraps
-- the seed admin on startup from SEED_ADMIN_EMAIL / SEED_ADMIN_PASSWORD
-- (see backend/src/auth.rs::ensure_seed_admin), hashing the password with
-- Argon2id before it ever touches the database.

CREATE EXTENSION IF NOT EXISTS "pgcrypto"; -- gen_random_uuid()

-- ---------------------------------------------------------------------------
-- Role enum
-- ---------------------------------------------------------------------------
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'user_role') THEN
        CREATE TYPE user_role AS ENUM ('admin', 'manager', 'learner');
    END IF;
END$$;

-- ---------------------------------------------------------------------------
-- updated_at trigger function
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ---------------------------------------------------------------------------
-- users
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS users (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email         TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role          user_role NOT NULL DEFAULT 'learner',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER trg_users_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- ---------------------------------------------------------------------------
-- trainings
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS trainings (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title            TEXT NOT NULL,
    description      TEXT NOT NULL DEFAULT '',
    duration_minutes INTEGER NOT NULL CHECK (duration_minutes >= 0),
    created_by       UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER trg_trainings_updated_at
    BEFORE UPDATE ON trainings
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- ---------------------------------------------------------------------------
-- certifications
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS certifications (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name             TEXT NOT NULL,
    issuing_body     TEXT NOT NULL,
    validity_months  INTEGER NOT NULL CHECK (validity_months > 0),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER trg_certifications_updated_at
    BEFORE UPDATE ON certifications
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- ---------------------------------------------------------------------------
-- user_training_completions
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS user_training_completions (
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    training_id  UUID NOT NULL REFERENCES trainings(id) ON DELETE CASCADE,
    completed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    score        INTEGER CHECK (score IS NULL OR (score >= 0 AND score <= 100)),
    PRIMARY KEY (user_id, training_id)
);

-- ---------------------------------------------------------------------------
-- user_certifications
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS user_certifications (
    user_id          UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    certification_id UUID NOT NULL REFERENCES certifications(id) ON DELETE CASCADE,
    issued_date      DATE NOT NULL,
    expiry_date      DATE NOT NULL,
    document_url     TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, certification_id)
);

CREATE TRIGGER trg_user_certifications_updated_at
    BEFORE UPDATE ON user_certifications
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- ---------------------------------------------------------------------------
-- sessions — server-side session store
-- Only a SHA-256 hash of the token is stored, so a database leak does not
-- expose usable session tokens. Rows are deleted on logout and ignored once
-- expires_at has passed.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS sessions (
    token_hash TEXT PRIMARY KEY,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at);
CREATE INDEX IF NOT EXISTS idx_completions_user ON user_training_completions(user_id);
CREATE INDEX IF NOT EXISTS idx_user_certs_user ON user_certifications(user_id);
CREATE INDEX IF NOT EXISTS idx_user_certs_expiry ON user_certifications(expiry_date);
