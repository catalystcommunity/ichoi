// Owner controls for an account-owned browser share: open it to every signed-in account,
// or limit it to a checklist of accounts. Each change saves at once.
import { createResource, createSignal, For, Show, type JSX } from "solid-js";
import { useI18n } from "../lib/i18n.tsx";
import { useServers } from "../stores/servers.tsx";
import { CsilServiceError } from "../lib/csil.ts";
import type { Player, ShareAccess, ShareAccessInfo } from "../lib/schema.ts";
import { Dialog } from "./Dialog.tsx";
import { Spinner } from "./common.tsx";

interface Props {
  player: Player | undefined;
  onClose: () => void;
}

export function ShareAccessDialog(props: Props): JSX.Element {
  const servers = useServers();
  const { t } = useI18n();
  const [error, setError] = createSignal<string>();
  const [saving, setSaving] = createSignal(false);

  const [info, { mutate }] = createResource(
    () => {
      const api = servers.api();
      const player = props.player;
      return api && player ? { api, id: player.id } : undefined;
    },
    ({ api, id }) => api.player.getShareAccess({ player_id: id }),
  );

  const save = async (access: ShareAccess, accountIds: string[]) => {
    const api = servers.api();
    const current = info();
    if (!api || !current) return;
    setError(undefined);
    setSaving(true);
    try {
      mutate(
        await api.player.setShareAccess({
          player_id: current.player_id,
          access,
          account_ids: accountIds,
        }),
      );
    } catch (err) {
      setError(err instanceof CsilServiceError ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const selectedIds = (current: ShareAccessInfo) =>
    current.accounts.filter((account) => account.selected).map((account) => account.account_id);

  const toggle = (current: ShareAccessInfo, accountId: string, checked: boolean) => {
    const others = selectedIds(current).filter((id) => id !== accountId);
    void save(current.access, checked ? [...others, accountId] : others);
  };

  return (
    <Dialog
      open={props.player !== undefined}
      title={t("jukebox.shareAccessTitle", { name: props.player?.name ?? "" })}
      onClose={props.onClose}
    >
      <Show when={info()} fallback={<Spinner label={t("common.loading")} />}>
        {(current) => (
          <div class="field">
            <p class="hint">{t("jukebox.shareAccessHint")}</p>
            <label class="row">
              <input
                type="radio"
                name="share-access"
                checked={current().access === "everyone"}
                disabled={saving()}
                onChange={() => void save("everyone", selectedIds(current()))}
              />
              {t("jukebox.shareAccessEveryone")}
            </label>
            <label class="row">
              <input
                type="radio"
                name="share-access"
                checked={current().access === "selected"}
                disabled={saving()}
                onChange={() => void save("selected", selectedIds(current()))}
              />
              {t("jukebox.shareAccessSelected")}
            </label>
            <Show when={current().access === "selected"}>
              <Show
                when={current().accounts.length > 0}
                fallback={<p class="hint">{t("jukebox.shareAccessNobody")}</p>}
              >
                <div style={{ display: "grid", gap: "6px", "margin-left": "24px" }}>
                  <For each={current().accounts}>
                    {(account) => (
                      <label class="row">
                        <input
                          type="checkbox"
                          checked={account.selected}
                          disabled={saving()}
                          onChange={(event) =>
                            toggle(current(), account.account_id, event.currentTarget.checked)
                          }
                        />
                        {account.display_name ?? account.handle}
                      </label>
                    )}
                  </For>
                </div>
                <Show when={selectedIds(current()).length === 0}>
                  <p class="hint">{t("jukebox.shareAccessNoneSelected")}</p>
                </Show>
              </Show>
            </Show>
          </div>
        )}
      </Show>
      <Show when={error() ?? (info.error ? String(info.error) : undefined)}>
        {(message) => (
          <p class="hint" style={{ color: "var(--danger)" }}>
            {message()}
          </p>
        )}
      </Show>
      <div class="dialog-actions">
        <button type="button" class="btn btn-primary" onClick={props.onClose}>
          {t("common.close")}
        </button>
      </div>
    </Dialog>
  );
}
