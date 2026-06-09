# LMS — Learning Management System

A Dockerized Learning Management System for tracking employee trainings and
certifications.

- **Backend:** Rust + [Axum](https://github.com/tokio-rs/axum), [SQLx](https://github.com/launchbadge/sqlx)
- **Database:** PostgreSQL 16
- **Frontend:** React + TypeScript (Vite), served by nginx
- **Orchestration:** Docker Compose

## Quick start

```bash
cp .env.example .env
# Edit .env: set strong POSTGRES_PASSWORD, SESSION_SECRET, and SEED_ADMIN_* values.
#   openssl rand -hex 32      # good for SESSION_SECRET
#   openssl rand -base64 24   # good for passwords

docker compose up --build
```

Then open <http://localhost:8080> and sign in with the `SEED_ADMIN_EMAIL` /
`SEED_ADMIN_PASSWORD` you configured in `.env`.

> **Local HTTP note:** session cookies are issued with the `Secure` attribute
> by default, which browsers only send over HTTPS. For local plain-HTTP testing
> set `COOKIE_SECURE=false` in `.env`. Keep it `true` in production (behind
> TLS).

## Architecture

```
            ┌─────────────┐        ┌──────────────┐        ┌──────────────┐
 browser ─▶ │  frontend   │ ─/api▶ │     app      │ ─SQL─▶ │     db       │
  :8080     │  (nginx)    │        │ (Rust/Axum)  │        │ (Postgres)   │
            └─────────────┘        └──────────────┘        └──────────────┘
              published              internal only            internal only
                                  (no host port)            (no host port)
```

Only the `frontend` service publishes a host port. nginx serves the built SPA
and reverse-proxies `/api/*` to the `app` container over the internal Docker
network. Postgres is reachable only by `app`.

## Project layout

```
.
├── docker-compose.yml          # three services + healthchecks + named volume
├── .env.example                # placeholder secrets (copy to .env)
├── db/
│   └── init.sql                # schema, enum, triggers, session store
├── backend/
│   ├── Dockerfile              # multi-stage build, runs as non-root
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs             # bootstrap, pool, graceful shutdown
│       ├── config.rs           # env-only configuration
│       ├── state.rs            # shared AppState
│       ├── error.rs            # AppError -> JSON responses
│       ├── models.rs           # DB row / response types
│       ├── auth.rs             # Argon2id, sessions, middleware, seed admin
│       ├── validation.rs       # input validation helpers
│       └── routes/             # auth, trainings, certifications, users, reports
└── frontend/
    ├── Dockerfile              # build with Node, serve with nginx (non-root)
    ├── nginx.conf              # SPA fallback + /api proxy + health endpoint
    └── src/                    # React + TypeScript (hooks only)
```

## API

All routes except `POST /api/auth/login` and `GET /health` require a valid
session cookie.

| Method | Path | Notes |
| ------ | ---- | ----- |
| POST | `/api/auth/login` | issue session cookie |
| POST | `/api/auth/logout` | invalidate session |
| GET | `/api/me` | current user profile |
| GET | `/api/trainings` | list trainings |
| POST | `/api/trainings` | create (admin/manager) |
| GET | `/api/trainings/:id` | training detail |
| PUT | `/api/trainings/:id` | update (admin/manager) |
| DELETE | `/api/trainings/:id` | delete (admin/manager) |
| POST | `/api/trainings/:id/complete` | log completion for current user |
| GET | `/api/certifications` | list certifications |
| POST | `/api/certifications` | create (admin/manager) |
| GET | `/api/certifications/:id` | certification detail |
| PUT | `/api/certifications/:id` | update (admin/manager) |
| DELETE | `/api/certifications/:id` | delete (admin/manager) |
| POST | `/api/certifications/:id/assign` | assign cert to a user (admin/manager) |
| GET | `/api/users` | list users (admin/manager) |
| GET | `/api/users/:id/dashboard` | completions + cert status (own, or any for admin/manager) |
| GET | `/api/reports/compliance` | overdue/expiring certs (admin only) |

## Security notes

- **SQL:** every query uses parameterized statements (SQLx bind parameters);
  no string interpolation of user input into SQL.
- **Passwords:** hashed with **Argon2id**; never stored or logged in plaintext.
- **Sessions:** 256-bit random tokens; only their SHA-256 hash is stored
  server-side with an expiry; deleted on logout.
- **Cookies:** `HttpOnly`, `Secure` (configurable for local dev),
  `SameSite=Strict`.
- **Input validation:** max lengths and range checks on every endpoint;
  request bodies reject unknown fields (`deny_unknown_fields`).
- **Containers:** both `app` and `frontend` run as non-root users; the only
  writable mount is the named Postgres volume (init.sql is mounted read-only).
- **Secrets:** all secrets come from environment variables; nothing is
  hardcoded. The seed admin is created on first run from `SEED_ADMIN_*`, so no
  credentials live in source or in `init.sql`.
- **Network:** Postgres publishes no host port; only `app` can reach it.

## Data protection (GDPR)

The system is designed to support the core data-subject rights and the
security/storage-limitation principles of the GDPR:

- **Data minimisation:** only an email, role, and Argon2id password hash are
  stored per user. Emails are not written to application logs.
- **Right of access (Art. 15):** `GET /api/users/:id/dashboard` returns the
  subject's full record — profile, training completions, and certification
  status. Users can read their own; admins/managers can read any.
- **Right to erasure (Art. 17):** `DELETE /api/users/:id` (admin) removes the
  account and cascades to the user's sessions, training completions, and
  certification records. Authored trainings are retained as organisational
  content but their `created_by` link is nulled (`ON DELETE SET NULL`), so no
  personal identifier remains. Deleting yourself or the last admin is blocked
  to prevent lockout.
- **Security of processing (Art. 32):** passwords hashed with Argon2id; session
  tokens are 256-bit random values stored only as a keyed HMAC-SHA256 digest;
  cookies are `HttpOnly` / `Secure` / `SameSite=Strict`; login is rate-limited
  per account against brute force; all traffic to the database stays on the
  internal Docker network.
- **Storage limitation:** expired sessions are purged hourly by a background
  task rather than retained indefinitely.

For a production deployment you should additionally serve everything over TLS,
define a documented retention policy for completion/certification history, and
present a privacy notice / capture lawful basis at the application level.

## Local development (without Docker)

Backend:

```bash
cd backend
export DATABASE_URL=postgres://lms:lms@localhost:5432/lms
export SESSION_SECRET=$(openssl rand -hex 32)
export SEED_ADMIN_EMAIL=admin@example.com SEED_ADMIN_PASSWORD=devpassword
export COOKIE_SECURE=false
cargo run
```

Frontend:

```bash
cd frontend
npm install
npm run dev   # proxies /api to http://localhost:8080
```
