import { createResource, createSignal, For, Show, type JSX } from "solid-js";
import { changeAccountBlock, isSignedIn, loadAllBlockedAccounts } from "../lib/compliance.ts";
import { useServers } from "../stores/servers.tsx";

export function BlockedAccounts(): JSX.Element {
  const servers = useServers();
  const [error, setError] = createSignal<string>();
  const [accounts, { refetch }] = createResource(
    () => {
      const api = servers.api();
      const activeId = servers.activeId();
      return api && activeId && isSignedIn(servers.active()?.session) ? { api, activeId } : undefined;
    },
    ({ api }) => loadAllBlockedAccounts((page) => api.library.listBlockedAccounts(page)),
  );

  async function unblock(accountId: string): Promise<void> {
    const api = servers.api();
    if (!api) return;
    setError(undefined);
    try {
      await changeAccountBlock(
        (request) => api.library.unblockAccount(request),
        () => refetch(),
        accountId,
      );
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    }
  }

  return (
    <Show when={isSignedIn(servers.active()?.session)}>
      <section class="panel" aria-labelledby="blocked-accounts-title">
        <h2 id="blocked-accounts-title">Blocked accounts</h2>
        <p class="hint">
          A block hides public playlists from that account on this selected server.
        </p>
        <Show when={!accounts.loading && (accounts()?.length ?? 0) === 0}>
          <p class="hint">You have not blocked an account on this server.</p>
        </Show>
        <div class="settings-list">
          <For each={accounts() ?? []}>
            {(account) => (
              <div class="row spread settings-row">
                <span>
                  <strong>{account.display_name ?? account.handle}</strong>
                  <br />
                  <span class="hint">{account.account_id}</span>
                </span>
                <button
                  class="btn btn-ghost"
                  type="button"
                  onClick={() => void unblock(account.account_id)}
                >
                  Unblock
                </button>
              </div>
            )}
          </For>
        </div>
        <Show when={error()}>{(message) => <p class="error">{message()}</p>}</Show>
      </section>
    </Show>
  );
}
