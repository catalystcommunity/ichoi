import { createEffect, createSignal, For, Show, type JSX } from "solid-js";
import {
  buildReportRequest,
  REPORT_REASONS,
  submitContentReport,
} from "../lib/compliance.ts";
import type { ContentReportReason, ContentReportTargetType } from "../lib/schema.ts";
import { useServers } from "../stores/servers.tsx";
import { useToast } from "../stores/toasts.tsx";
import { Dialog } from "./Dialog.tsx";

interface Props {
  open: boolean;
  targetType: ContentReportTargetType;
  targetId: string;
  targetLabel: string;
  onClose: () => void;
}

function reasonLabel(reason: ContentReportReason): string {
  return reason.split("-").map((word) => word[0]!.toUpperCase() + word.slice(1)).join(" ");
}

export function ReportDialog(props: Props): JSX.Element {
  const servers = useServers();
  const toast = useToast();
  const [reason, setReason] = createSignal<ContentReportReason>("objectionable-content");
  const [details, setDetails] = createSignal("");
  const [error, setError] = createSignal<string>();
  const [submitting, setSubmitting] = createSignal(false);

  createEffect(() => {
    if (props.open) {
      setReason("objectionable-content");
      setDetails("");
      setError(undefined);
    }
  });

  async function submit(event: Event): Promise<void> {
    event.preventDefault();
    const api = servers.api();
    if (!api) return;
    setError(undefined);
    setSubmitting(true);
    try {
      const request = buildReportRequest(props.targetType, props.targetId, reason(), details());
      await submitContentReport((value) => api.library.reportContent(value), request);
      toast.show("Report sent to this server's administrators.");
      props.onClose();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Dialog open={props.open} title={`Report ${props.targetLabel}`} onClose={props.onClose}>
      <form onSubmit={(event) => void submit(event)}>
        <div class="field">
          <label for="report-reason">Reason</label>
          <select
            id="report-reason"
            class="select"
            value={reason()}
            onChange={(event) => setReason(event.currentTarget.value as ContentReportReason)}
          >
            <For each={REPORT_REASONS}>
              {(value) => <option value={value}>{reasonLabel(value)}</option>}
            </For>
          </select>
        </div>
        <div class="field">
          <label for="report-details">Details (optional)</label>
          <textarea
            id="report-details"
            class="input"
            rows={5}
            value={details()}
            onInput={(event) => setDetails(event.currentTarget.value)}
          />
          <p class="hint">{[...details()].length}/2,000 Unicode characters</p>
        </div>
        <Show when={error()}>{(message) => <p class="error">{message()}</p>}</Show>
        <div class="dialog-actions">
          <button type="button" class="btn btn-ghost" onClick={props.onClose}>Cancel</button>
          <button type="submit" class="btn btn-primary" disabled={submitting()}>
            {submitting() ? "Sending…" : "Send report"}
          </button>
        </div>
      </form>
    </Dialog>
  );
}
