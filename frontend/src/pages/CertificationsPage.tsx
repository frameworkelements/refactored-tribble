import { useEffect, useState, type FormEvent } from "react";
import { api } from "../api";
import { useAuth } from "../auth/AuthContext";
import { StatusBadge } from "../components/StatusBadge";
import type { Certification, CertificationStatus } from "../types";

export function CertificationsPage() {
  const { user } = useAuth();
  const canManage = user?.role === "admin" || user?.role === "manager";

  const [catalogue, setCatalogue] = useState<Certification[]>([]);
  const [mine, setMine] = useState<CertificationStatus[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [showForm, setShowForm] = useState(false);

  const [name, setName] = useState("");
  const [issuingBody, setIssuingBody] = useState("");
  const [validity, setValidity] = useState(12);
  const [saving, setSaving] = useState(false);

  function load() {
    api
      .listCertifications()
      .then(setCatalogue)
      .catch((e) => setError(e instanceof Error ? e.message : "Failed to load"));
    if (user) {
      api
        .dashboard(user.id)
        .then((d) => setMine(d.certifications))
        .catch(() => {
          /* dashboard errors surfaced on the dashboard page */
        });
    }
  }

  useEffect(load, [user]);

  async function onCreate(e: FormEvent) {
    e.preventDefault();
    setError(null);
    setSaving(true);
    try {
      await api.createCertification({
        name,
        issuing_body: issuingBody,
        validity_months: Number(validity),
      });
      setName("");
      setIssuingBody("");
      setValidity(12);
      setShowForm(false);
      load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create");
    } finally {
      setSaving(false);
    }
  }

  return (
    <>
      <div className="topbar">
        <h2>Certifications</h2>
        {canManage && (
          <button onClick={() => setShowForm((s) => !s)}>
            {showForm ? "Cancel" : "New certification"}
          </button>
        )}
      </div>

      {error && <div className="error">{error}</div>}

      {showForm && canManage && (
        <form className="card" onSubmit={onCreate}>
          <h3>Create certification</h3>
          <label>Name</label>
          <input value={name} maxLength={200} onChange={(e) => setName(e.target.value)} required />
          <label>Issuing body</label>
          <input
            value={issuingBody}
            maxLength={200}
            onChange={(e) => setIssuingBody(e.target.value)}
            required
          />
          <label>Validity (months)</label>
          <input
            type="number"
            min={1}
            value={validity}
            onChange={(e) => setValidity(Number(e.target.value))}
            required
          />
          <button type="submit" disabled={saving}>
            {saving ? "Saving…" : "Create"}
          </button>
        </form>
      )}

      <div className="card">
        <h3>My certification status</h3>
        {mine.length === 0 ? (
          <p className="muted">No certifications assigned to you.</p>
        ) : (
          <table>
            <thead>
              <tr>
                <th>Certification</th>
                <th>Issued</th>
                <th>Expiry</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {mine.map((c) => (
                <tr key={c.certification_id}>
                  <td>{c.name}</td>
                  <td>{c.issued_date}</td>
                  <td>{c.expiry_date}</td>
                  <td>
                    <StatusBadge status={c.status} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      <div className="card">
        <h3>Catalogue</h3>
        <table>
          <thead>
            <tr>
              <th>Name</th>
              <th>Issuing body</th>
              <th>Validity (months)</th>
            </tr>
          </thead>
          <tbody>
            {catalogue.map((c) => (
              <tr key={c.id}>
                <td>{c.name}</td>
                <td>{c.issuing_body}</td>
                <td>{c.validity_months}</td>
              </tr>
            ))}
            {catalogue.length === 0 && (
              <tr>
                <td colSpan={3} className="muted">
                  No certifications defined yet.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </>
  );
}
