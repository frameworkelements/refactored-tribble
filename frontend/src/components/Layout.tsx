import { NavLink } from "react-router-dom";
import type { ReactNode } from "react";
import { useAuth } from "../auth/AuthContext";

export function Layout({ children }: { children: ReactNode }) {
  const { user, logout } = useAuth();

  return (
    <div className="layout">
      <nav className="sidebar">
        <h1>LMS</h1>
        <NavLink to="/" end className="nav-link">
          Dashboard
        </NavLink>
        <NavLink to="/trainings" className="nav-link">
          Trainings
        </NavLink>
        <NavLink to="/certifications" className="nav-link">
          Certifications
        </NavLink>
        {user?.role === "admin" && (
          <NavLink to="/admin" className="nav-link">
            Admin
          </NavLink>
        )}
        <div className="spacer" />
        <div className="muted" style={{ fontSize: "0.8rem", marginBottom: "0.5rem" }}>
          {user?.email}
          <br />
          <em>{user?.role}</em>
        </div>
        <button className="secondary" onClick={() => void logout()}>
          Log out
        </button>
      </nav>
      <main className="content">{children}</main>
    </div>
  );
}
