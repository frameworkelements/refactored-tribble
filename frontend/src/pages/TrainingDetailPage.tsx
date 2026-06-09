import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { api } from "../api";
import { useAuth } from "../auth/AuthContext";
import type { Training } from "../types";

export function TrainingDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { user } = useAuth();
  const canManage = user?.role === "admin" || user?.role === "manager";

  const [training, setTraining] = useState<Training | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [score, setScore] = useState<string>("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!id) return;
    api
      .getTraining(id)
      .then(setTraining)
      .catch((e) => setError(e instanceof Error ? e.message : "Not found"));
  }, [id]);

  async function markComplete() {
    if (!id) return;
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const parsed = score.trim() === "" ? undefined : Number(score);
      await api.completeTraining(id, parsed);
      setMessage("Marked as complete.");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed");
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!id) return;
    if (!confirm("Delete this training?")) return;
    try {
      await api.deleteTraining(id);
      navigate("/trainings");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed");
    }
  }

  if (error) return <div className="error">{error}</div>;
  if (!training) return <div className="center">Loading…</div>;

  return (
    <>
      <div className="topbar">
        <h2>{training.title}</h2>
        {canManage && (
          <button className="danger" onClick={() => void remove()}>
            Delete
          </button>
        )}
      </div>

      <div className="card">
        <p className="muted">{training.duration_minutes} minutes</p>
        <p>{training.description || "No description."}</p>
      </div>

      <div className="card">
        <h3>Mark as complete</h3>
        {message && <p style={{ color: "var(--green)" }}>{message}</p>}
        <label>Score (optional, 0–100)</label>
        <input
          type="number"
          min={0}
          max={100}
          value={score}
          onChange={(e) => setScore(e.target.value)}
        />
        <button onClick={() => void markComplete()} disabled={busy}>
          {busy ? "Saving…" : "Mark complete"}
        </button>
      </div>
    </>
  );
}
