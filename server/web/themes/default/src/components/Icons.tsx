// Inline SVG icons. Decorative by default (aria-hidden); pair with a text label
// or an aria-label on the interactive parent. Stroke inherits `currentColor`.
import type { JSX } from "solid-js";

type IconProps = { class?: string; size?: number };

function base(props: IconProps, children: JSX.Element): JSX.Element {
  return (
    <svg
      class={`nav-ico ${props.class ?? ""}`}
      width={props.size ?? 18}
      height={props.size ?? 18}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.7"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      {children}
    </svg>
  );
}

export const IconLibrary = (p: IconProps) =>
  base(p, (
    <>
      <path d="M4 5v14" />
      <path d="M8 5v14" />
      <rect x="11" y="5" width="4" height="14" rx="1" />
      <path d="M18 6l3 12" />
    </>
  ));

export const IconSearch = (p: IconProps) =>
  base(p, (
    <>
      <circle cx="11" cy="11" r="7" />
      <path d="m20 20-3.2-3.2" />
    </>
  ));

export const IconPlaylist = (p: IconProps) =>
  base(p, (
    <>
      <path d="M4 7h11" />
      <path d="M4 12h11" />
      <path d="M4 17h7" />
      <circle cx="17" cy="16" r="3" />
      <path d="M20 16V9l-3 1" />
    </>
  ));

export const IconJukebox = (p: IconProps) =>
  base(p, (
    <>
      <rect x="4" y="3" width="16" height="18" rx="3" />
      <circle cx="12" cy="10" r="3.2" />
      <path d="M8 17h8" />
    </>
  ));

export const IconNowPlaying = (p: IconProps) =>
  base(p, (
    <>
      <circle cx="12" cy="12" r="8" />
      <circle cx="12" cy="12" r="1.6" />
    </>
  ));

export const IconSettings = (p: IconProps) =>
  base(p, (
    <>
      <circle cx="12" cy="12" r="3" />
      <path d="M19 12a7 7 0 0 0-.1-1.2l2-1.6-2-3.4-2.4 1a7 7 0 0 0-2-1.2L14 2h-4l-.5 2.6a7 7 0 0 0-2 1.2l-2.4-1-2 3.4 2 1.6A7 7 0 0 0 5 12a7 7 0 0 0 .1 1.2l-2 1.6 2 3.4 2.4-1a7 7 0 0 0 2 1.2L10 22h4l.5-2.6a7 7 0 0 0 2-1.2l2.4 1 2-3.4-2-1.6A7 7 0 0 0 19 12Z" />
    </>
  ));

export const IconPlay = (p: IconProps) => base(p, <path d="M8 5.5v13l11-6.5-11-6.5Z" fill="currentColor" stroke="none" />);
export const IconPause = (p: IconProps) =>
  base(p, (
    <>
      <rect x="7" y="5" width="3.4" height="14" rx="1" fill="currentColor" stroke="none" />
      <rect x="13.6" y="5" width="3.4" height="14" rx="1" fill="currentColor" stroke="none" />
    </>
  ));
export const IconNext = (p: IconProps) =>
  base(p, (
    <>
      <path d="M6 6l9 6-9 6V6Z" fill="currentColor" stroke="none" />
      <rect x="16.5" y="6" width="2" height="12" rx="1" fill="currentColor" stroke="none" />
    </>
  ));
export const IconPrev = (p: IconProps) =>
  base(p, (
    <>
      <path d="M18 6l-9 6 9 6V6Z" fill="currentColor" stroke="none" />
      <rect x="5.5" y="6" width="2" height="12" rx="1" fill="currentColor" stroke="none" />
    </>
  ));
export const IconStop = (p: IconProps) =>
  base(p, <rect x="6" y="6" width="12" height="12" rx="2" fill="currentColor" stroke="none" />);

export const IconPlus = (p: IconProps) =>
  base(p, (
    <>
      <path d="M12 5v14" />
      <path d="M5 12h14" />
    </>
  ));

export const IconChevronLeft = (p: IconProps) => base(p, <path d="m14 6-6 6 6 6" />);
export const IconVolume = (p: IconProps) =>
  base(p, (
    <>
      <path d="M4 9v6h4l5 4V5L8 9H4Z" />
      <path d="M17 9a4 4 0 0 1 0 6" />
    </>
  ));
export const IconBroadcast = (p: IconProps) =>
  base(p, (
    <>
      <circle cx="12" cy="12" r="2" />
      <path d="M8.5 8.5a5 5 0 0 0 0 7M15.5 8.5a5 5 0 0 1 0 7" />
      <path d="M6 6a9 9 0 0 0 0 12M18 6a9 9 0 0 1 0 12" />
    </>
  ));
