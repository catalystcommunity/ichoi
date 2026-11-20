/** Player icons: inline SVG on a 24px grid. Each icon uses `currentColor`. */
const solid = (d: string) => `<path d="${d}" fill="currentColor"/>`;
const line = (...d: string[]) => d.map(path => `<path d="${path}"/>`).join("");

const ICONS = {
  play: solid("M8 5.5v13a1 1 0 0 0 1.5.86l10.2-6.5a1 1 0 0 0 0-1.72L9.5 4.64A1 1 0 0 0 8 5.5z"),
  pause: solid("M6 5h4v14H6zM14 5h4v14h-4z"),
  stop: solid("M6.5 6.5h11v11h-11z"),
  previous: solid("M6 5h2.5v14H6zM19 5.8v12.4a.8.8 0 0 1-1.25.66L9.6 12.66a.8.8 0 0 1 0-1.32l8.15-6.2A.8.8 0 0 1 19 5.8z"),
  next: solid("M15.5 5H18v14h-2.5zM5 5.8v12.4a.8.8 0 0 0 1.25.66l8.15-6.2a.8.8 0 0 0 0-1.32l-8.15-6.2A.8.8 0 0 0 5 5.8z"),
  shuffle: line("M16 4h4v4", "M4 19 20 4", "M20 16v4h-4", "M14.5 14.5 20 20", "M4 5l5 5"),
  repeat: line("M17 2.5 20.5 6 17 9.5", "M3.5 11V9.5A3.5 3.5 0 0 1 7 6h13.5", "M7 21.5 3.5 18 7 14.5", "M20.5 13v1.5A3.5 3.5 0 0 1 17 18H3.5"),
  repeatOne: line("M17 2.5 20.5 6 17 9.5", "M3.5 11V9.5A3.5 3.5 0 0 1 7 6h13.5", "M7 21.5 3.5 18 7 14.5", "M20.5 13v1.5A3.5 3.5 0 0 1 17 18H3.5", "M11 10.5l1.5-1v5"),
  playThrough: line("M3.5 12h13", "M12.5 7.5 17 12l-4.5 4.5", "M20.5 5.5v13"),
  queue: line("M3.5 6h13", "M3.5 11h13", "M3.5 16h7") + solid("M15 14v7l5.5-3.5z"),
  volume: solid("M3.5 9.5h3.8L12 5.6v12.8l-4.7-3.9H3.5z") + line("M15.5 9a4.2 4.2 0 0 1 0 6", "M18.2 6.3a8 8 0 0 1 0 11.4"),
  volumeLow: solid("M3.5 9.5h3.8L12 5.6v12.8l-4.7-3.9H3.5z") + line("M15.5 9a4.2 4.2 0 0 1 0 6"),
  muted: solid("M3.5 9.5h3.8L12 5.6v12.8l-4.7-3.9H3.5z") + line("M15.5 9.5l5 5", "M20.5 9.5l-5 5"),
  speaker: `<rect x="5.5" y="2.5" width="13" height="19" rx="2.5"/><circle cx="12" cy="14.5" r="3.5"/>` + solid("M12 5.6a1.4 1.4 0 1 1 0 2.8 1.4 1.4 0 0 1 0-2.8z"),
  more: solid("M5 10.2a1.8 1.8 0 1 1 0 3.6 1.8 1.8 0 0 1 0-3.6zm7 0a1.8 1.8 0 1 1 0 3.6 1.8 1.8 0 0 1 0-3.6zm7 0a1.8 1.8 0 1 1 0 3.6 1.8 1.8 0 0 1 0-3.6z"),
  close: line("M6 6l12 12", "M18 6 6 18"),
  add: line("M12 5v14", "M5 12h14"),
  playNext: line("M3.5 7h9", "M3.5 12h9", "M3.5 17h6") + solid("M15 4v8l6-4z"),
  up: line("M6 15l6-6 6 6"),
  down: line("M6 9l6 6 6-6"),
} as const;

export type IconName = keyof typeof ICONS;

/** An SVG element for `name`, hidden from screen readers (the control carries the name). */
export function icon(name: IconName): SVGSVGElement {
  const template = document.createElement("template");
  template.innerHTML = `<svg class="icon" viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">${ICONS[name]}</svg>`;
  return template.content.firstElementChild as SVGSVGElement;
}
