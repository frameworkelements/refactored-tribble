import { useEffect, useState, type FormEvent } from "react";
import { api } from "../api";
import { StatusBadge } from "../components/StatusBadge";
import type { Certification, ComplianceEntry, UserProfile } from "../types";

export function AdminPage() {
  const [users, setUsers] = useState<UserProfile[]>([]);
  const [certs, setCerts] = useState<Certification[]>([]);
  const [report, setReport] = useState<ComplianceEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  // assign form
  const [userId, setUserId] = useState("");
  const [certId, setCertId] = useState("");
  const [issued, setIssued] = useState("");
  const [expiry, setExpiry] = useState("");
  const [docUrl, setDocUrl] = useState("");
  const [saving, setSaving] = useState(false);

  function load() {
    Promise.all([api.listUsers(), api.listCertifications(), api.compliance()])
      .then(([u, c, r]) => {
        setUsers(u);
        setCerts(c);
        setReport(r);
      })
      .catch((e) => setError(e instanceof Error ? e.message : "Failed to load"));
  }

  useEffect(load, []);

  async function onAssign(e: FormEvent) {
    e.preventDefault();
    setError(null);
    setMessage(null);
    setSaving(true);
    try {
      await api.assignCertification(certId, {
        user_id: userId,
        issued_date: issued,
        expiry_date: expiry,
        document_url: docUrl.trim() || undefined,
      });
      setMessage("Certification assigned.");
      setDocUrl("");
      load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to assign");
    } finally {
      setSaving(false);
    }
  }

  return (
    <>
      <div className="topbar">
        <h2>Admin panel</h2>
      </div>

      {error && <div className="error">{error}</div>}
      {message && <p style={{ color: "var(--green)" }}>{message}</p>}

      <div className="card">
        <h3>Assign certification to user</h3>
        <form onSubmit={onAssign}>
          <label>User</label>
          <select value={userId} onChange={(e) => setUserId(e.target.value)} required>
            <option value="">Select a user…</option>
            {users.map((u) => (
              <option key={u.id} value={u.id}>
                {u.email} ({u.role})
              </option>
            ))}
          </select>

          <label>Certification</label>
          <select value={certId} onChange={(e) => setCertId(e.target.value)} required>
            <option value="">Select a certification…</option>
            {certs.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name} — {c.issuing_body}
              </option>
            ))}
          </select>

          <div className="row">
            <div style={{ flex: 1 }}>
              <label>Issued date</label>
              <input
                type="date"
                value={issued}
                onChange={(e) => setIssued(e.target.value)}
                required
              />
            </div>
            <div style={{ flex: 1 }}>
              <label>Expiry date</label>
              <input
                type="date"
                value={expiry}
                onChange={(e) => setExpiry(e.target.value)}
                required
              />
            </div>
          </div>

          <label>Document URL (optional)</label>
          <input
            type="url"
            value={docUrl}
            maxLength={2048}
            onChange={(e) => setDocUrl(e.target.value)}
          />

          <button type="submit" disabled={saving}>
            {saving ? "Assigning…" : "Assign certification"}
          </button>
        </form>
      </div>

      <div className="card">
        <h3>Users</h3>
        <table>
          <thead>
            <tr>
              <th>Email</th>
              <th>Role</th>
              <th>Created</th>
            </tr>
          </thead>
          <tbody>
            {users.map((u) => (
              <tr key={u.id}>
                <td>{u.email}</td>
                <td>{u.role}</td>
                <td>{new Date(u.created_at).toLocaleDateString()}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="card">
        <h3>Compliance report</h3>
        <p className="muted">
          Users with certifications that are expired or expiring within 30 days.
        </p>
        {report.length === 0 ? (
          <p style={{ color: "var(--green)" }}>Everyone is compliant. 🎉</p>
        ) : (
          report.map((entry) => (
            <div key={entry.user.id} style={{ marginBottom: "1rem" }}>
              <strong>{entry.user.email}</strong>
              <table>
                <thead>
                  <tr>
                    <th>Certification</th>
                    <th>Expiry</th>
                    <th>Days</th>
                    <th>Status</th>
                  </tr>
                </thead>
                <tbody>
                  {entry.certifications.map((c) => (
                    <tr key={c.certification_id}>
                      <td>{c.name}</td>
                      <td>{c.expiry_date}</td>
                      <td>
                        {c.days_to_expiry < 0
                          ? `${Math.abs(c.days_to_expiry)} overdue`
                          : c.days_to_expiry}
                      </td>
                      <td>
                        <StatusBadge status={c.status} />
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ))
        )}
      </div>
    </>
  );
}
