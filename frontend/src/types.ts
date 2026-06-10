export type Role = "admin" | "manager" | "learner";

export interface UserProfile {
  id: string;
  email: string;
  role: Role;
  created_at: string;
}

export interface Training {
  id: string;
  title: string;
  description: string;
  duration_minutes: number;
  created_by: string;
  created_at: string;
  updated_at: string;
}

export interface Certification {
  id: string;
  name: string;
  issuing_body: string;
  validity_months: number;
  created_at: string;
  updated_at: string;
}

export interface CompletionRecord {
  training_id: string;
  title: string;
  completed_at: string;
  score: number | null;
}

export type CertStatus = "valid" | "expiring_soon" | "expired";

export interface CertificationStatus {
  certification_id: string;
  name: string;
  issuing_body: string;
  issued_date: string;
  expiry_date: string;
  document_url: string | null;
  days_to_expiry: number;
  status: CertStatus;
}

export interface Dashboard {
  user: UserProfile;
  completions: CompletionRecord[];
  certifications: CertificationStatus[];
}

export interface ComplianceEntry {
  user: UserProfile;
  certifications: CertificationStatus[];
}

export interface TrainingSession {
  id: string;
  training_id: string;
  training_title: string;
  starts_at: string;
  ends_at: string;
  location: string;
  instructor: string | null;
  capacity: number | null;
  enrolled_count: number;
  enrolled: boolean;
}

export interface SessionEnrollee {
  user_id: string;
  email: string;
  status: string;
  enrolled_at: string;
}
