import { useEffect, useState, type FormEvent } from "react";
import { Link } from "react-router-dom";
import { api } from "../api";
import { useAuth } from "../auth/AuthContext";
import { formatTimeRange, groupByDay } from "../format";
import type { Training, TrainingSession } from "../types";

export function SchedulePage() {
  const { user } = useAuth();
  const canManage = user?.role === "admin" || user?.role === "manager";

  const [sessions, setSessions] = useState<TrainingSession[]>([]);
  const [trainings, setTrainings] = useState<Training[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [showForm, setShowForm] = useState(false);

  // create-session form
  const [trainingId, setTrainingId] = useState("");
  const [starts, setStarts] = useState("");
  const [ends, setEnds] = useState("");
  const [location, setLocation] = useState("");
  const [instructor, setInstructor] = useState("");
  const [capacity, setCapacity] = useState("");
  const [saving, setSaving] = useState(false);

  function load() {
    api
      .listSessions({ upcoming: true })
      .then(setSessions)
      .catch((e) => setError(e instanceof Error ? e.message : "Failed to load"));
    if (canManage) {
      api.listTrainings().then(setTrainings).catch(() => undefined);
    }
  }

  useEffect(load, [canManage]);

  async function onCreate(e: FormEvent) {
    e.preventDefault();
    setError(null);
    setSaving(true);
    try {
      await api.createSession({
        training_id: trainingId,
        starts_at: new Date(starts).toISOString(),
        ends_at: new Date(ends).toISOString(),
        location: location.trim() || undefined,
        instructor: instructor.trim() || undefined,
        capacity: capacity.trim() === "" ? null : Number(capacity),
      });
      setShowForm(false);
      setStarts("");
      setEnds("");
      setLocation("");
      setInstructor("");
      setCapacity("");
      load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create session");
    } finally {
      setSaving(false);
    }
  }

  async function toggleEnroll(s: TrainingSession) {
    setError(null);
    setBusyId(s.id);
    try {
      if (s.enrolled) {
        await api.cancelSession(s.id);
      } else {
        await api.enrollSession(s.id);
      }
      load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Action failed");
    } finally {
      setBusyId(null);
    }
  }

  const groups = groupByDay(sessions, (s) => s.starts_at);

  return (
    <>
      <div className="topbar">
        <h2>Schedule</h2>
        {canManage && (
          <button onClick={() => setShowForm((s) => !s)}>
            {showForm ? "Cancel" : "Schedule a session"}
          </button>
        )}
      </div>

      {error && <div className="error">{error}</div>}

      {showForm && canManage && (
        <form className="card" onSubmit={onCreate}>
          <h3>Schedule a session</h3>
          <label>Training</label>
          <select value={trainingId} onChange={(e) => setTrainingId(e.target.value)} required>
            <option value="">Select a training…</option>
            {trainings.map((t) => (
              <option key={t.id} value={t.id}>
                {t.title}
              </option>
            ))}
          </select>
          <div className="row">
            <div style={{ flex: 1 }}>
              <label>Starts</label>
              <input
                type="datetime-local"
                value={starts}
                onChange={(e) => setStarts(e.target.value)}
                required
              />
            </div>
            <div style={{ flex: 1 }}>
              <label>Ends</label>
              <input
                type="datetime-local"
                value={ends}
                onChange={(e) => setEnds(e.target.value)}
                required
              />
            </div>
          </div>
          <div className="row">
            <div style={{ flex: 2 }}>
              <label>Location</label>
              <input
                value={location}
                maxLength={200}
                placeholder="Room 4 / Online"
                onChange={(e) => setLocation(e.target.value)}
              />
            </div>
            <div style={{ flex: 2 }}>
              <label>Instructor</label>
              <input
                value={instructor}
                maxLength={120}
                onChange={(e) => setInstructor(e.target.value)}
              />
            </div>
            <div style={{ flex: 1 }}>
              <label>Capacity</label>
              <input
                type="number"
                min={1}
                placeholder="∞"
                value={capacity}
                onChange={(e) => setCapacity(e.target.value)}
              />
            </div>
          </div>
          <button type="submit" disabled={saving}>
            {saving ? "Scheduling…" : "Schedule session"}
          </button>
        </form>
      )}

      {sessions.length === 0 ? (
        <div className="card center">No upcoming sessions scheduled.</div>
      ) : (
        groups.map((g) => (
          <div key={g.key} style={{ marginBottom: "1.5rem" }}>
            <h3 className="day-heading">{g.day}</h3>
            {g.items.map((s) => {
              const full =
                s.capacity !== null && s.enrolled_count >= s.capacity && !s.enrolled;
              return (
                <div key={s.id} className="card session-card">
                  <div className="session-time">{formatTimeRange(s.starts_at, s.ends_at)}</div>
                  <div className="session-main">
                    <Link to={`/trainings/${s.training_id}`} className="session-title">
                      {s.training_title}
                    </Link>
                    <div className="muted session-meta">
                      {s.location && <span>📍 {s.location}</span>}
                      {s.instructor && <span>👤 {s.instructor}</span>}
                      <span>
                        🎟️ {s.enrolled_count}
                        {s.capacity !== null ? ` / ${s.capacity}` : ""} enrolled
                      </span>
                    </div>
                  </div>
                  <div className="session-action">
                    {s.enrolled && <span className="badge valid">Enrolled</span>}
                    <button
                      className={s.enrolled ? "secondary" : ""}
                      disabled={busyId === s.id || full}
                      onClick={() => void toggleEnroll(s)}
                    >
                      {full
                        ? "Full"
                        : s.enrolled
                        ? "Cancel"
                        : busyId === s.id
                        ? "…"
                        : "Enroll"}
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        ))
      )}
    </>
  );
}
