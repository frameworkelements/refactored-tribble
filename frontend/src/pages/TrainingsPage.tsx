import { useEffect, useState, type FormEvent } from "react";
import { Link } from "react-router-dom";
import { api } from "../api";
import { useAuth } from "../auth/AuthContext";
import type { Training } from "../types";

export function TrainingsPage() {
  const { user } = useAuth();
  const canManage = user?.role === "admin" || user?.role === "manager";

  const [trainings, setTrainings] = useState<Training[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [showForm, setShowForm] = useState(false);

  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [duration, setDuration] = useState(30);
  const [saving, setSaving] = useState(false);

  function load() {
    api
      .listTrainings()
      .then(setTrainings)
      .catch((e) => setError(e instanceof Error ? e.message : "Failed to load"));
  }

  useEffect(load, []);

  async function onCreate(e: FormEvent) {
    e.preventDefault();
    setError(null);
    setSaving(true);
    try {
      await api.createTraining({
        title,
        description,
        duration_minutes: Number(duration),
      });
      setTitle("");
      setDescription("");
      setDuration(30);
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
        <h2>Training catalogue</h2>
        {canManage && (
          <button onClick={() => setShowForm((s) => !s)}>
            {showForm ? "Cancel" : "New training"}
          </button>
        )}
      </div>

      {error && <div className="error">{error}</div>}

      {showForm && canManage && (
        <form className="card" onSubmit={onCreate}>
          <h3>Create training</h3>
          <label>Title</label>
          <input
            value={title}
            maxLength={200}
            onChange={(e) => setTitle(e.target.value)}
            required
          />
          <label>Description</label>
          <textarea
            value={description}
            maxLength={5000}
            rows={3}
            onChange={(e) => setDescription(e.target.value)}
          />
          <label>Duration (minutes)</label>
          <input
            type="number"
            min={0}
            value={duration}
            onChange={(e) => setDuration(Number(e.target.value))}
            required
          />
          <button type="submit" disabled={saving}>
            {saving ? "Saving…" : "Create"}
          </button>
        </form>
      )}

      <div className="grid">
        {trainings.map((t) => (
          <Link to={`/trainings/${t.id}`} key={t.id} className="card">
            <h3 style={{ marginTop: 0 }}>{t.title}</h3>
            <p className="muted">{t.duration_minutes} min</p>
            <p>{t.description.slice(0, 120) || "No description."}</p>
          </Link>
        ))}
        {trainings.length === 0 && <p className="muted">No trainings yet.</p>}
      </div>
    </>
  );
}
