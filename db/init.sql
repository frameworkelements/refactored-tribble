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
    -- Nullable: SSO-provisioned accounts have no local password.
    password_hash TEXT,
    role          user_role NOT NULL DEFAULT 'learner',
    -- Stable external identity for SSO (issuer + subject claim).
    oidc_issuer   TEXT,
    oidc_subject  TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- One local identity per (issuer, subject) pair.
CREATE UNIQUE INDEX IF NOT EXISTS uq_users_oidc
    ON users (oidc_issuer, oidc_subject)
    WHERE oidc_issuer IS NOT NULL AND oidc_subject IS NOT NULL;

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
    -- Nullable + ON DELETE SET NULL so a user can be erased (GDPR Art. 17)
    -- without destroying the organisational training content they authored;
    -- the personal link is severed instead.
    created_by       UUID REFERENCES users(id) ON DELETE SET NULL,
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

-- ---------------------------------------------------------------------------
-- oidc_auth_requests — short-lived, one-time store for in-flight SSO logins.
-- Holds the PKCE verifier and nonce keyed by the CSRF state value. Rows are
-- consumed (deleted) on callback and ignored once expired.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS oidc_auth_requests (
    state         TEXT PRIMARY KEY,
    pkce_verifier TEXT NOT NULL,
    nonce         TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at    TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_oidc_requests_expires ON oidc_auth_requests(expires_at);
CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at);
CREATE INDEX IF NOT EXISTS idx_completions_user ON user_training_completions(user_id);
CREATE INDEX IF NOT EXISTS idx_user_certs_user ON user_certifications(user_id);
CREATE INDEX IF NOT EXISTS idx_user_certs_expiry ON user_certifications(expiry_date);

-- ---------------------------------------------------------------------------
-- training_sessions — scheduled instances of a training (date/time, place,
-- optional instructor and capacity). Deleting a training removes its sessions.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS training_sessions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    training_id UUID NOT NULL REFERENCES trainings(id) ON DELETE CASCADE,
    starts_at   TIMESTAMPTZ NOT NULL,
    ends_at     TIMESTAMPTZ NOT NULL,
    location    TEXT NOT NULL DEFAULT '',
    instructor  TEXT,
    -- NULL capacity means unlimited seats.
    capacity    INTEGER CHECK (capacity IS NULL OR capacity > 0),
    created_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (ends_at > starts_at)
);

CREATE TRIGGER trg_training_sessions_updated_at
    BEFORE UPDATE ON training_sessions
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- ---------------------------------------------------------------------------
-- session_enrollments — who is signed up for a scheduled session.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS session_enrollments (
    session_id  UUID NOT NULL REFERENCES training_sessions(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status      TEXT NOT NULL DEFAULT 'enrolled'
                CHECK (status IN ('enrolled', 'cancelled', 'attended')),
    enrolled_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (session_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_sessions_training ON training_sessions(training_id);
CREATE INDEX IF NOT EXISTS idx_sessions_starts_at ON training_sessions(starts_at);
CREATE INDEX IF NOT EXISTS idx_enrollments_user ON session_enrollments(user_id);
