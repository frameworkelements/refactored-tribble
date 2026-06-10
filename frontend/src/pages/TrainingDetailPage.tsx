import { useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { api } from "../api";
import { useAuth } from "../auth/AuthContext";
import { formatDay, formatTimeRange } from "../format";
import type { Training, TrainingSession } from "../types";

export function TrainingDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { user } = useAuth();
  const canManage = user?.role === "admin" || user?.role === "manager";

  const [training, setTraining] = useState<Training | null>(null);
  const [sessions, setSessions] = useState<TrainingSession[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [score, setScore] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);

  function loadSessions(trainingId: string) {
    api
      .listSessions({ trainingId, upcoming: true })
      .then(setSessions)
      .catch(() => undefined);
  }

  useEffect(() => {
    if (!id) return;
    api
      .getTraining(id)
      .then(setTraining)
      .catch((e) => setError(e instanceof Error ? e.message : "Not found"));
    loadSessions(id);
  }, [id]);

  async function toggleEnroll(s: TrainingSession) {
    setBusyId(s.id);
    setError(null);
    try {
      if (s.enrolled) await api.cancelSession(s.id);
      else await api.enrollSession(s.id);
      if (id) loadSessions(id);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Action failed");
    } finally {
      setBusyId(null);
    }
  }

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
        <div className="topbar" style={{ marginBottom: "0.5rem" }}>
          <h3 style={{ margin: 0 }}>Upcoming sessions</h3>
          {canManage && <Link to="/schedule">Schedule one →</Link>}
        </div>
        {sessions.length === 0 ? (
          <p className="muted">No upcoming sessions for this training.</p>
        ) : (
          <ul className="schedule-list">
            {sessions.map((s) => {
              const full =
                s.capacity !== null && s.enrolled_count >= s.capacity && !s.enrolled;
              return (
                <li key={s.id}>
                  <span>
                    {formatDay(s.starts_at)} · {formatTimeRange(s.starts_at, s.ends_at)}
                    {s.location ? ` · ${s.location}` : ""}
                    <span className="muted">
                      {"  "}
                      ({s.enrolled_count}
                      {s.capacity !== null ? `/${s.capacity}` : ""} enrolled)
                    </span>
                  </span>
                  <button
                    className={s.enrolled ? "secondary" : ""}
                    disabled={busyId === s.id || full}
                    onClick={() => void toggleEnroll(s)}
                  >
                    {full ? "Full" : s.enrolled ? "Cancel" : "Enroll"}
                  </button>
                </li>
              );
            })}
          </ul>
        )}
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
