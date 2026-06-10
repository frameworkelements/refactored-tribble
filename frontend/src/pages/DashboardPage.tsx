import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { api } from "../api";
import { useAuth } from "../auth/AuthContext";
import { StatusBadge } from "../components/StatusBadge";
import { formatDay, formatTimeRange } from "../format";
import type { Dashboard, TrainingSession } from "../types";

export function DashboardPage() {
  const { user } = useAuth();
  const [data, setData] = useState<Dashboard | null>(null);
  const [schedule, setSchedule] = useState<TrainingSession[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!user) return;
    api
      .dashboard(user.id)
      .then(setData)
      .catch((e) => setError(e instanceof Error ? e.message : "Failed to load"));
    api
      .mySchedule()
      .then(setSchedule)
      .catch(() => undefined);
  }, [user]);

  if (error) return <div className="error">{error}</div>;
  if (!data) return <div className="center">Loading dashboard…</div>;

  return (
    <>
      <div className="topbar">
        <h2>My dashboard</h2>
      </div>

      <div className="card">
        <h3>My upcoming sessions</h3>
        {schedule.length === 0 ? (
          <p className="muted">
            No sessions booked. Browse the <Link to="/schedule">schedule</Link> to enroll.
          </p>
        ) : (
          <ul className="schedule-list">
            {schedule.map((s) => (
              <li key={s.id}>
                <Link to={`/trainings/${s.training_id}`}>{s.training_title}</Link>
                <span className="muted">
                  {formatDay(s.starts_at)} · {formatTimeRange(s.starts_at, s.ends_at)}
                  {s.location ? ` · ${s.location}` : ""}
                </span>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="card">
        <h3>Certifications</h3>
        {data.certifications.length === 0 ? (
          <p className="muted">No certifications assigned yet.</p>
        ) : (
          <table>
            <thead>
              <tr>
                <th>Certification</th>
                <th>Issuing body</th>
                <th>Expiry</th>
                <th>Days left</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {data.certifications.map((c) => (
                <tr key={c.certification_id}>
                  <td>{c.name}</td>
                  <td>{c.issuing_body}</td>
                  <td>{c.expiry_date}</td>
                  <td
                    style={{
                      color:
                        c.status === "expired"
                          ? "var(--red)"
                          : c.status === "expiring_soon"
                          ? "var(--amber)"
                          : "var(--text)",
                      fontWeight: c.status === "valid" ? 400 : 600,
                    }}
                  >
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
        )}
      </div>

      <div className="card">
        <h3>Completed trainings</h3>
        {data.completions.length === 0 ? (
          <p className="muted">No completed trainings yet.</p>
        ) : (
          <table>
            <thead>
              <tr>
                <th>Training</th>
                <th>Completed</th>
                <th>Score</th>
              </tr>
            </thead>
            <tbody>
              {data.completions.map((c) => (
                <tr key={c.training_id}>
                  <td>{c.title}</td>
                  <td>{new Date(c.completed_at).toLocaleDateString()}</td>
                  <td>{c.score ?? "—"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </>
  );
}
