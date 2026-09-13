import { createSignal, Show, type JSX } from "solid-js";
import { deleteCurrentAccount, isSignedIn, PRIVACY_URL } from "../lib/compliance.ts";
import { useServers } from "../stores/servers.tsx";
import { Dialog } from "./Dialog.tsx";

export function AccountSettings(): JSX.Element {
  const servers = useServers();
  const [open, setOpen] = createSignal(false);
  const [confirmation, setConfirmation] = createSignal("");
  const [error, setError] = createSignal<string>();
  const [deleting, setDeleting] = createSignal(false);
  const session = () => servers.active()?.session;

  async function removeAccount(event: Event): Promise<void> {
    event.preventDefault();
    const api = servers.api();
    const current = session();
    if (!api || !current) return;
    setError(undefined);
    setDeleting(true);
    try {
      await deleteCurrentAccount(
        (handle) => api.session.deleteAccount({ confirmation_handle: handle }),
        servers.clearLocalSession,
        current.handle,
        confirmation(),
      );
      setOpen(false);
      setConfirmation("");
    } catch (caught) {
      const detail = caught instanceof Error ? caught.message : String(caught);
      setError(
        `The account was not deleted. Your session is still stored. Contact the administrator of ${servers.active()?.url ?? "the selected server"}. ${detail}`,
      );
    } finally {
      setDeleting(false);
    }
  }

  return (
    <section class="panel" aria-labelledby="account-settings-title">
      <h2 id="account-settings-title">Account</h2>
      <p><a href={PRIVACY_URL} target="_blank" rel="noreferrer">Privacy policy</a></p>
      <div class="row" style={{ gap: "10px", "flex-wrap": "wrap" }}>
        <button type="button" class="btn" onClick={() => void servers.signOut()}>Sign out</button>
        <Show when={isSignedIn(session())}>
          <button type="button" class="btn" onClick={() => setOpen(true)}>Delete account</button>
        </Show>
      </div>
      <Dialog
        open={open()}
        title="Delete account"
        onClose={() => !deleting() && setOpen(false)}
      >
        <p>
          This action deletes your account from the selected server. It deletes your private
          playlists. It does not delete public playlists that you created.
        </p>
        <p class="hint">Selected server: <span class="mono">{servers.active()?.url}</span></p>
        <form onSubmit={(event) => void removeAccount(event)}>
          <div class="field">
            <label for="delete-account-confirmation">
              Enter <strong>{session()?.handle}</strong> to confirm
            </label>
            <input
              id="delete-account-confirmation"
              class="input"
              autocomplete="off"
              value={confirmation()}
              onInput={(event) => setConfirmation(event.currentTarget.value)}
            />
          </div>
          <Show when={error()}>{(message) => <p class="error">{message()}</p>}</Show>
          <div class="dialog-actions">
            <button type="button" class="btn btn-ghost" disabled={deleting()} onClick={() => setOpen(false)}>Cancel</button>
            <button type="submit" class="btn" disabled={deleting()}>{deleting() ? "Deleting…" : "Delete account"}</button>
          </div>
        </form>
      </Dialog>
    </section>
  );
}
