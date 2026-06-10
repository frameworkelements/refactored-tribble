import { Navigate, Route, Routes } from "react-router-dom";
import { useAuth } from "./auth/AuthContext";
import { Layout } from "./components/Layout";
import { LoginPage } from "./pages/LoginPage";
import { DashboardPage } from "./pages/DashboardPage";
import { TrainingsPage } from "./pages/TrainingsPage";
import { TrainingDetailPage } from "./pages/TrainingDetailPage";
import { CertificationsPage } from "./pages/CertificationsPage";
import { AdminPage } from "./pages/AdminPage";

export default function App() {
  const { user, loading } = useAuth();

  if (loading) {
    return <div className="center">Loading…</div>;
  }

  if (!user) {
    return (
      <Routes>
        <Route path="/login" element={<LoginPage />} />
        <Route path="*" element={<Navigate to="/login" replace />} />
      </Routes>
    );
  }

  return (
    <Layout>
      <Routes>
        <Route path="/" element={<DashboardPage />} />
        <Route path="/trainings" element={<TrainingsPage />} />
        <Route path="/trainings/:id" element={<TrainingDetailPage />} />
        <Route path="/certifications" element={<CertificationsPage />} />
        {user.role === "admin" && <Route path="/admin" element={<AdminPage />} />}
        <Route path="/login" element={<Navigate to="/" replace />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </Layout>
  );
}
