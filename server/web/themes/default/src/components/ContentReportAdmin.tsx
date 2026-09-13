import { createResource, createSignal, For, Show, type JSX } from "solid-js";
import { A } from "@solidjs/router";
import {
  changeReportStatus,
  deleteReportedTarget,
  loadAllAccounts,
  loadAllContentReports,
  reportPage,
} from "../lib/compliance.ts";
import type { ContentReport } from "../lib/schema.ts";
import { useServers } from "../stores/servers.tsx";
import { EmptyState, Spinner } from "./common.tsx";

const PAGE_SIZE = 20;

export function ContentReportAdmin(): JSX.Element {
  const servers = useServers();
  const [offset, setOffset] = createSignal(0);
  const [message, setMessage] = createSignal<string>();
  const [data, { refetch }] = createResource(
    () => servers.active()?.session?.can_admin ? servers.api() : undefined,
    async (api) => {
      const [reports, accounts] = await Promise.all([
        loadAllContentReports((page) => api!.admin.listContentReports(page)),
        loadAllAccounts((page) => api!.admin.listAccounts(page)),
      ]);
      return { reports, accounts };
    },
  );
  const visible = () => reportPage(data()?.reports ?? [], offset(), PAGE_SIZE);
  const accountHandle = (id: string) => data()?.accounts.find((account) => account.id === id)?.handle;

  async function update(report: ContentReport, status: "resolved" | "dismissed"): Promise<void> {
    const api = servers.api();
    if (!api) return;
    setMessage(undefined);
    try {
      await changeReportStatus(
        (request) => api.admin.updateContentReportStatus(request),
        report.id,
        status,
      );
      await refetch();
    } catch (caught) {
      setMessage(caught instanceof Error ? caught.message : String(caught));
    }
  }

  async function deleteTarget(report: ContentReport): Promise<void> {
    const api = servers.api();
    if (!api) return;
    const confirmed = window.confirm(`Delete the reported ${report.target_type}? This cannot be undone.`);
    if (!confirmed) return;
    setMessage(undefined);
    try {
      await deleteReportedTarget(
        report,
        {
          deletePlaylist: (playlistId) => api.library.deletePlaylist({ playlist_id: playlistId }),
          deleteAccount: (accountId, handle) => api.admin.deleteAccount({
            account_id: accountId,
            confirmation_handle: handle,
          }),
        },
        accountHandle(report.target_id),
      );
      await refetch();
    } catch (caught) {
      setMessage(caught instanceof Error ? caught.message : String(caught));
    }
  }

  return (
    <section class="panel" aria-labelledby="content-reports-title">
      <h2 id="content-reports-title">Content reports</h2>
      <p class="hint">Review reports for this server. Open reports appear first.</p>
      <Show when={!data.loading} fallback={<Spinner label="Loading reports…" />}>
        <Show when={visible().length > 0} fallback={<EmptyState title="No content reports" />}>
          <div class="settings-list">
            <For each={visible()}>
              {(report) => (
                <article class="settings-row" aria-label={`Report ${report.id}`}>
                  <div class="row spread">
                    <strong>{report.reason}</strong>
                    <span class="chip">{report.status}</span>
                  </div>
                  <p class="hint">
                    {report.target_type}: <span class="mono">{report.target_id}</span><br />
                    Reporter: <span class="mono">{report.reporter_account_id}</span>
                  </p>
                  <Show when={report.details}><p>{report.details}</p></Show>
                  <div class="row" style={{ gap: "8px", "flex-wrap": "wrap" }}>
                    <Show when={report.target_type === "playlist"}>
                      <A class="btn btn-ghost" href="/playlists">Open playlists</A>
                    </Show>
                    <button type="button" class="btn btn-ghost" onClick={() => void deleteTarget(report)}>
                      Delete {report.target_type}
                    </button>
                    <button type="button" class="btn" onClick={() => void update(report, "resolved")}>Resolve</button>
                    <button type="button" class="btn" onClick={() => void update(report, "dismissed")}>Dismiss</button>
                  </div>
                </article>
              )}
            </For>
          </div>
          <div class="row spread" style={{ "margin-top": "12px" }}>
            <button class="btn btn-ghost" type="button" disabled={offset() === 0} onClick={() => setOffset(Math.max(0, offset() - PAGE_SIZE))}>Previous</button>
            <span class="hint">{offset() + 1}–{Math.min(offset() + PAGE_SIZE, data()?.reports.length ?? 0)} of {data()?.reports.length ?? 0}</span>
            <button class="btn btn-ghost" type="button" disabled={offset() + PAGE_SIZE >= (data()?.reports.length ?? 0)} onClick={() => setOffset(offset() + PAGE_SIZE)}>Next</button>
          </div>
        </Show>
      </Show>
      <Show when={message()}>{(text) => <p class="error">{text()}</p>}</Show>
    </section>
  );
}
