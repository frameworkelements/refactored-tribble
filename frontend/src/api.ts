import type {
  Certification,
  CertificationStatus,
  ComplianceEntry,
  Dashboard,
  Role,
  SessionEnrollee,
  Training,
  TrainingSession,
  UserProfile,
} from "./types";

export interface SessionInput {
  training_id: string;
  starts_at: string;
  ends_at: string;
  location?: string;
  instructor?: string;
  capacity?: number | null;
}

export class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

async function request<T>(
  method: string,
  path: string,
  body?: unknown
): Promise<T> {
  const res = await fetch(path, {
    method,
    credentials: "include",
    headers: body ? { "Content-Type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  });

  if (res.status === 204) {
    return undefined as T;
  }

  const text = await res.text();
  const data = text ? JSON.parse(text) : undefined;

  if (!res.ok) {
    const message =
      data && typeof data === "object" && "error" in data
        ? (data as { error: string }).error
        : `request failed (${res.status})`;
    throw new ApiError(res.status, message);
  }

  return data as T;
}

/// URL the browser navigates to in order to begin the SSO flow.
export const SSO_LOGIN_URL = "/api/auth/sso/login";

export const api = {
  // auth
  login: (email: string, password: string) =>
    request<UserProfile>("POST", "/api/auth/login", { email, password }),
  logout: () => request<void>("POST", "/api/auth/logout"),
  me: () => request<UserProfile>("GET", "/api/me"),
  ssoStatus: () =>
    request<{ enabled: boolean }>("GET", "/api/auth/sso/status"),

  // trainings
  listTrainings: () => request<Training[]>("GET", "/api/trainings"),
  getTraining: (id: string) => request<Training>("GET", `/api/trainings/${id}`),
  createTraining: (input: {
    title: string;
    description?: string;
    duration_minutes: number;
  }) => request<Training>("POST", "/api/trainings", input),
  updateTraining: (
    id: string,
    input: { title: string; description?: string; duration_minutes: number }
  ) => request<Training>("PUT", `/api/trainings/${id}`, input),
  deleteTraining: (id: string) =>
    request<void>("DELETE", `/api/trainings/${id}`),
  completeTraining: (id: string, score?: number) =>
    request<void>("POST", `/api/trainings/${id}/complete`, {
      score: score ?? null,
    }),

  // certifications
  listCertifications: () =>
    request<Certification[]>("GET", "/api/certifications"),
  getCertification: (id: string) =>
    request<Certification>("GET", `/api/certifications/${id}`),
  createCertification: (input: {
    name: string;
    issuing_body: string;
    validity_months: number;
  }) => request<Certification>("POST", "/api/certifications", input),
  updateCertification: (
    id: string,
    input: { name: string; issuing_body: string; validity_months: number }
  ) => request<Certification>("PUT", `/api/certifications/${id}`, input),
  deleteCertification: (id: string) =>
    request<void>("DELETE", `/api/certifications/${id}`),
  assignCertification: (
    id: string,
    input: {
      user_id: string;
      issued_date: string;
      expiry_date: string;
      document_url?: string;
    }
  ) => request<void>("POST", `/api/certifications/${id}/assign`, input),

  // sessions (scheduling)
  listSessions: (opts?: { trainingId?: string; upcoming?: boolean }) => {
    const params = new URLSearchParams();
    if (opts?.trainingId) params.set("training_id", opts.trainingId);
    if (opts?.upcoming !== undefined) params.set("upcoming", String(opts.upcoming));
    const qs = params.toString();
    return request<TrainingSession[]>("GET", `/api/sessions${qs ? `?${qs}` : ""}`);
  },
  getSession: (id: string) => request<TrainingSession>("GET", `/api/sessions/${id}`),
  createSession: (input: SessionInput) =>
    request<TrainingSession>("POST", "/api/sessions", input),
  updateSession: (id: string, input: SessionInput) =>
    request<TrainingSession>("PUT", `/api/sessions/${id}`, input),
  deleteSession: (id: string) => request<void>("DELETE", `/api/sessions/${id}`),
  enrollSession: (id: string) => request<void>("POST", `/api/sessions/${id}/enroll`),
  cancelSession: (id: string) => request<void>("POST", `/api/sessions/${id}/cancel`),
  sessionEnrollments: (id: string) =>
    request<SessionEnrollee[]>("GET", `/api/sessions/${id}/enrollments`),
  mySchedule: () => request<TrainingSession[]>("GET", "/api/me/schedule"),

  // users & reports
  listUsers: () => request<UserProfile[]>("GET", "/api/users"),
  createUser: (input: { email: string; password: string; role: Role }) =>
    request<UserProfile>("POST", "/api/users", input),
  deleteUser: (id: string) => request<void>("DELETE", `/api/users/${id}`),
  dashboard: (id: string) =>
    request<Dashboard>("GET", `/api/users/${id}/dashboard`),
  compliance: () => request<ComplianceEntry[]>("GET", "/api/reports/compliance"),
};

export type { CertificationStatus };
