import type {
  Account,
  ContentReport,
  ContentReportReason,
  ContentReportStatus,
  ContentReportTargetType,
  ReportContentRequest,
  SessionInfo,
} from "./schema.ts";

export const TERMS_STORAGE_KEY = "ichoi.terms.accepted.v1";
export const TERMS_URL = "https://ichoi.invalid/terms/";
export const PRIVACY_URL = "https://ichoi.invalid/privacy/";
export const REPORT_REASONS: readonly ContentReportReason[] = [
  "objectionable-content",
  "harassment",
  "spam",
  "other",
];

interface StorageReader {
  getItem(key: string): string | null;
}

interface StorageWriter extends StorageReader {
  setItem(key: string, value: string): void;
}

export function hasAcceptedTerms(storage?: StorageReader): boolean {
  try {
    const target = storage ?? globalThis.localStorage;
    return target.getItem(TERMS_STORAGE_KEY) === "accepted";
  } catch {
    return false;
  }
}

export function acceptTerms(storage?: StorageWriter): void {
  const target = storage ?? globalThis.localStorage;
  target.setItem(TERMS_STORAGE_KEY, "accepted");
}

export function isSignedIn(session?: SessionInfo): boolean {
  return Boolean(session && session.account_id !== "__guest__");
}

export function canManageReports(session?: SessionInfo): boolean {
  return session?.can_admin === true;
}

export function requireTermsForPlaylist(storage?: StorageReader): void {
  if (!hasAcceptedTerms(storage)) {
    throw new Error("Accept the terms before you create a playlist.");
  }
}

export function validateDeleteConfirmation(currentHandle: string, enteredHandle: string): void {
  if (enteredHandle !== currentHandle) {
    throw new Error(`Enter ${currentHandle} exactly to delete this account.`);
  }
}

export async function deleteCurrentAccount(
  request: (confirmationHandle: string) => Promise<unknown>,
  clearLocalSession: () => Promise<void>,
  currentHandle: string,
  enteredHandle: string,
): Promise<void> {
  validateDeleteConfirmation(currentHandle, enteredHandle);
  await request(enteredHandle);
  await clearLocalSession();
}

export function buildReportRequest(
  targetType: ContentReportTargetType,
  targetId: string,
  reason: ContentReportReason,
  details: string,
): ReportContentRequest {
  if (!targetId.trim()) throw new Error("Select content to report.");
  if (!REPORT_REASONS.includes(reason)) throw new Error("Select a report reason.");
  const cleanDetails = details.trim();
  if ([...cleanDetails].length > 2_000) {
    throw new Error("Report details must contain 2,000 characters or fewer.");
  }
  return {
    target_type: targetType,
    target_id: targetId,
    reason,
    ...(cleanDetails ? { details: cleanDetails } : {}),
  };
}

export async function submitContentReport(
  submit: (request: ReportContentRequest) => Promise<ContentReport>,
  request: ReportContentRequest,
): Promise<ContentReport> {
  return submit(request);
}

export function sortReportsOpenFirst(reports: readonly ContentReport[]): ContentReport[] {
  return [...reports].sort((left, right) => {
    if (left.status === right.status) {
      return String(right.created_at).localeCompare(String(left.created_at));
    }
    if (left.status === "open") return -1;
    if (right.status === "open") return 1;
    return left.status.localeCompare(right.status);
  });
}

export function reportPage(
  reports: readonly ContentReport[],
  offset: number,
  limit: number,
): ContentReport[] {
  return sortReportsOpenFirst(reports).slice(offset, offset + limit);
}

export async function loadAllContentReports(
  load: (page: { offset: number; limit: number }) => Promise<{ reports: ContentReport[]; total: number }>,
): Promise<ContentReport[]> {
  const reports: ContentReport[] = [];
  const limit = 1_000;
  for (;;) {
    const page = await load({ offset: reports.length, limit });
    reports.push(...page.reports);
    if (reports.length >= page.total || page.reports.length === 0) {
      return sortReportsOpenFirst(reports);
    }
  }
}

export async function loadAllAccounts(
  load: (page: { offset: number; limit: number }) => Promise<{ accounts: Account[] }>,
): Promise<Account[]> {
  const accounts: Account[] = [];
  const limit = 1_000;
  for (;;) {
    const page = await load({ offset: accounts.length, limit });
    accounts.push(...page.accounts);
    if (page.accounts.length < limit) return accounts;
  }
}

export function reportStatusRequest(
  reportId: string,
  status: Exclude<ContentReportStatus, "open">,
): { report_id: string; status: ContentReportStatus } {
  return { report_id: reportId, status };
}

export async function changeReportStatus(
  update: (request: { report_id: string; status: ContentReportStatus }) => Promise<ContentReport>,
  reportId: string,
  status: Exclude<ContentReportStatus, "open">,
): Promise<ContentReport> {
  return update(reportStatusRequest(reportId, status));
}

export async function deleteReportedTarget(
  report: ContentReport,
  actions: {
    deletePlaylist: (playlistId: string) => Promise<unknown>;
    deleteAccount: (accountId: string, confirmationHandle: string) => Promise<unknown>;
  },
  confirmationHandle?: string,
): Promise<void> {
  if (report.target_type === "playlist") {
    await actions.deletePlaylist(report.target_id);
    return;
  }
  if (!confirmationHandle) throw new Error("The reported account is no longer available.");
  await actions.deleteAccount(report.target_id, confirmationHandle);
}
