"use client";

import { useEffect, useState } from "react";

const KEY = "theme";

/** Runs in <head> before paint so a stored choice never flashes the other theme. */
export const themeInitScript = `try{var t=localStorage.getItem("${KEY}");if(t==="light"||t==="dark")document.documentElement.dataset.theme=t}catch(e){}`;

function isDark() {
  const t = document.documentElement.dataset.theme;
  return t ? t === "dark" : window.matchMedia("(prefers-color-scheme: dark)").matches;
}

/** True when the page is dark, from the toggle or, without a choice, the OS setting. */
export function useDarkTheme() {
  const [dark, setDark] = useState(false);
  useEffect(() => {
    const sync = () => setDark(isDark());
    sync();
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener("change", sync);
    const obs = new MutationObserver(sync);
    obs.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => { mq.removeEventListener("change", sync); obs.disconnect(); };
  }, []);
  return dark;
}

export function ThemeToggle() {
  const dark = useDarkTheme();
  const toggle = () => {
    const next = dark ? "light" : "dark";
    document.documentElement.dataset.theme = next;
    try { localStorage.setItem(KEY, next); } catch {}
  };
  return (
    <button type="button" className="icon-btn theme-toggle" aria-pressed={dark} aria-label="Dark mode" title={dark ? "Switch to light mode" : "Switch to dark mode"} onClick={toggle}>
      <svg width="20" height="20" viewBox="0 0 24 24" aria-hidden="true">
        <mask id="moon-mask">
          <rect x="-4" y="-4" width="32" height="32" fill="#fff" />
          <circle className="moon-cut" cx="30" cy="-6" r="7" fill="#000" />
        </mask>
        <circle className="sun-core" cx="12" cy="12" r="5" fill="currentColor" mask="url(#moon-mask)" />
        <g className="sun-rays" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
          <path d="M12 1.5v2M12 20.5v2M1.5 12h2M20.5 12h2M4.6 4.6l1.4 1.4M18 18l1.4 1.4M4.6 19.4 6 18M18 6l1.4-1.4" />
        </g>
      </svg>
    </button>
  );
}
