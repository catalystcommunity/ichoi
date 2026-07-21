import { Show, type JSX } from "solid-js";
import { useNavigate } from "@solidjs/router";
import type { Album } from "../lib/schema.ts";
import { useI18n } from "../lib/i18n.tsx";
import { CoverArt } from "./CoverArt.tsx";

/** An album in a grid. The whole tile is a button that opens the album detail. */
export function AlbumTile(props: { album: Album; artistName?: string; href?: string }): JSX.Element {
  const navigate = useNavigate();
  const { t } = useI18n();
  const artistName = () => props.artistName ?? props.album.artist_name;
  return (
    <button
      type="button"
      class="tile"
      onClick={() => navigate(props.href ?? `/album/${encodeURIComponent(props.album.id)}`)}
      aria-label={`${props.album.title}${artistName() ? `, ${artistName()}` : ""}`}
    >
      <CoverArt album={props.album} />
      <span>
        <span class="tile-title">{props.album.title}</span>
        <Show when={artistName()}>{(name) => <span class="tile-sub">{name()}</span>}</Show>
        <span class="tile-sub">
          {t("library.tracksCount", { count: props.album.track_count })}
        </span>
      </span>
    </button>
  ) as JSX.Element;
}
