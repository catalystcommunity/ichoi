import { createResource, createSignal, For, Show, type JSX } from "solid-js";
import type { Role } from "../lib/schema.ts";
import { useServers } from "../stores/servers.tsx";

export function AccountAccessAdmin(): JSX.Element {
  const servers = useServers();
  const [identity, setIdentity] = createSignal("");
  const [message, setMessage] = createSignal<string>();
  const [data, { refetch }] = createResource(
    () => servers.active()?.session?.can_admin ? servers.api() : undefined,
    async (api) => {
      const [accounts, trust] = await Promise.all([
        api!.admin.listAccounts(),
        api!.admin.listTrustedIdentities(),
      ]);
      return { accounts: accounts.accounts, identities: trust.identities };
    },
  );

  async function addIdentity(event: Event): Promise<void> {
    event.preventDefault();
    const value = identity().trim();
    const api = servers.api();
    if (!api || !value) return;
    setMessage(undefined);
    try {
      await api.admin.trustIdentity({ identity: value });
      setIdentity("");
      await refetch();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function revokeIdentity(selector: string): Promise<void> {
    const api = servers.api();
    if (!api) return;
    setMessage(undefined);
    try {
      await api.admin.revokeTrustedIdentity({ identity: selector });
      await refetch();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function setAccountRole(accountId: string, role: Role): Promise<void> {
    const api = servers.api();
    if (!api) return;
    setMessage(undefined);
    try {
      await api.admin.setRole({ account_id: accountId, role });
      await refetch();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
      await refetch();
    }
  }

  return (
    <section class="panel" aria-labelledby="account-access-title">
      <h2 id="account-access-title">Accounts and LinkKeys access</h2>
      <p class="hint">
        Trust a whole domain or one exact handle. Configured entries come from the deployment;
        entries added here remain in the database.
      </p>

      <h3>Trusted identities</h3>
      <form class="row" onSubmit={(event) => void addIdentity(event)}>
        <input
          class="input"
          aria-label="Domain or handle at domain"
          placeholder="family.example or alice@friends.example"
          value={identity()}
          onInput={(event) => setIdentity(event.currentTarget.value)}
        />
        <button class="btn" type="submit">Trust identity</button>
      </form>
      <div class="settings-list">
        <For each={data()?.identities ?? []}>
          {(entry) => {
            const selector = entry.handle ? `${entry.handle}@${entry.domain}` : entry.domain;
            return (
              <div class="row spread settings-row">
                <span>
                  <strong>{selector}</strong>
                  <br />
                  <span class="hint">
                    {entry.source === "config" ? "Deployment configuration" : "Added by an administrator"}
                  </span>
                </span>
                <button
                  class="btn btn-ghost"
                  type="button"
                  disabled={entry.source === "config"}
                  title={entry.source === "config" ? "Remove this from ICHOI_LINKKEYS_TRUSTED_IDENTITIES" : "Revoke"}
                  onClick={() => void revokeIdentity(selector)}
                >
                  Revoke
                </button>
              </div>
            );
          }}
        </For>
      </div>

      <h3>Accounts</h3>
      <div class="settings-list">
        <For each={data()?.accounts ?? []}>
          {(account) => (
            <div class="row spread settings-row">
              <span>
                <strong>{account.display_name ?? account.handle}</strong>
                <br />
                <span class="hint">{account.id}</span>
              </span>
              <select
                class="select"
                style={{ "max-width": "150px" }}
                value={account.role}
                aria-label={`Role for ${account.handle}`}
                onChange={(event) =>
                  void setAccountRole(account.id, event.currentTarget.value as Role)}
              >
                <option value="admin">Admin</option>
                <option value="member">Member</option>
                <option value="guest">Guest</option>
              </select>
            </div>
          )}
        </For>
      </div>
      <Show when={message()}>{(text) => <p class="error">{text()}</p>}</Show>
    </section>
  );
}
