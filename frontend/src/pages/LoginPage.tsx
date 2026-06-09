import { useEffect, useState, type FormEvent } from "react";
import { useAuth } from "../auth/AuthContext";
import { ApiError, api, SSO_LOGIN_URL } from "../api";

export function LoginPage() {
  const { login } = useAuth();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [ssoEnabled, setSsoEnabled] = useState(false);

  useEffect(() => {
    // Show the SSO button only if the backend has OIDC configured.
    api
      .ssoStatus()
      .then((s) => setSsoEnabled(s.enabled))
      .catch(() => setSsoEnabled(false));
    // Surface an error bounced back from a failed SSO round-trip.
    if (new URLSearchParams(window.location.search).has("sso_error")) {
      setError("Single sign-on failed. Please try again or use your password.");
    }
  }, []);

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      await login(email, password);
    } catch (err) {
      if (err instanceof ApiError && err.status === 401) {
        setError("Invalid email or password.");
      } else {
        setError(err instanceof Error ? err.message : "Login failed.");
      }
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="login-wrap">
      <form className="card login-card" onSubmit={onSubmit}>
        <h1>Sign in</h1>
        {error && <div className="error">{error}</div>}
        <label htmlFor="email">Email</label>
        <input
          id="email"
          type="email"
          autoComplete="username"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          required
        />
        <label htmlFor="password">Password</label>
        <input
          id="password"
          type="password"
          autoComplete="current-password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          required
        />
        <button type="submit" disabled={submitting} style={{ width: "100%" }}>
          {submitting ? "Signing in…" : "Sign in"}
        </button>

        {ssoEnabled && (
          <>
            <div className="sso-divider">
              <span>or</span>
            </div>
            <button
              type="button"
              className="secondary"
              style={{ width: "100%" }}
              onClick={() => {
                window.location.href = SSO_LOGIN_URL;
              }}
            >
              Sign in with SSO
            </button>
          </>
        )}
      </form>
    </div>
  );
}
