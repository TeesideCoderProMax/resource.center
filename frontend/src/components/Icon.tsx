/** Small inline stroke icons (no icon font or package). Decorative: always aria-hidden. */
const PATHS = {
  search: "M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16zM21 21l-4.3-4.3",
  bag: "M6 7h12l1 14H5L6 7zM9 7V5a3 3 0 0 1 6 0v2",
  grid: "M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h6v6h-6z",
  list: "M4 6h16M4 12h16M4 18h16",
  laptop: "M5 5h14v10H5zM2 19h20",
  tablet: "M7 3h10a1 1 0 0 1 1 1v16a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zM11 18h2",
  camera: "M3 8h4l2-3h6l2 3h4v11H3zM12 17a4 4 0 1 0 0-8 4 4 0 0 0 0 8z",
  headset: "M4 15v-3a8 8 0 0 1 16 0v3M4 15h3v5H4zM17 15h3v5h-3z",
  mic: "M12 3a3 3 0 0 1 3 3v6a3 3 0 0 1-6 0V6a3 3 0 0 1 3-3zM5 11a7 7 0 0 0 14 0M12 18v3",
  chip: "M7 7h10v10H7zM10 3v4M14 3v4M10 17v4M14 17v4M3 10h4M3 14h4M17 10h4M17 14h4",
  box: "M3 7l9-4 9 4v10l-9 4-9-4zM3 7l9 4 9-4M12 11v10",
  calendar: "M4 6h16v14H4zM4 10h16M8 3v4M16 3v4",
  pin: "M12 21s7-6.2 7-12a7 7 0 0 0-14 0c0 5.8 7 12 7 12zM12 11a2 2 0 1 0 0-4 2 2 0 0 0 0 4z",
  clock: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 7v5l3 2",
  check: "M5 12l5 5L20 7",
  shield: "M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z",
  warning: "M12 3l10 18H2zM12 10v5M12 18v.5",
  download: "M12 3v12M7 10l5 5 5-5M4 21h16",
  home: "M3 11l9-8 9 8M5 10v10h14V10",
  user: "M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM4 21a8 8 0 0 1 16 0",
  info: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 11v6M12 7.5v.5",
  dashboard: "M4 4h7v9H4zM13 4h7v5h-7zM13 11h7v9h-7zM4 15h7v5H4z",
  swap: "M7 7h13l-3-3M17 17H4l3 3",
  alert: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 7v6M12 16v.5",
  plus: "M12 5v14M5 12h14",
  trash: "M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13",
  chevronLeft: "M15 6l-6 6 6 6",
  chevronRight: "M9 6l6 6-6 6",
  refresh: "M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7",
} as const;

export type IconName = keyof typeof PATHS;

export function Icon({ name, size = 18 }: { name: IconName; size?: number }) {
  return (
    <svg className="icon" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={PATHS[name]} />
    </svg>
  );
}

/** Icon for a catalogue category; unknown categories get a generic box. */
export function categoryIcon(category: string): IconName {
  const c = category.toLowerCase();
  if (/laptop|notebook|workstation/.test(c)) return "laptop";
  if (/tablet|ipad/.test(c)) return "tablet";
  if (/camera|video|photo/.test(c)) return "camera";
  if (/vr|ar|headset/.test(c)) return "headset";
  if (/audio|mic|podcast/.test(c)) return "mic";
  if (/sensor|micro|lab|kit/.test(c)) return "chip";
  return "box";
}
