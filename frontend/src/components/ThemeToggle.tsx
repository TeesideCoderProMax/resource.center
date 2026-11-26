"use client";

import { useEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";

const KEY = "theme";
const RADIAL_DURATION_MS = 850;
const FALLBACK_DURATION_MS = 750;

/** Runs in <head> before paint so a stored choice never flashes the other theme. */
export const themeInitScript = `try{var t=localStorage.getItem("${KEY}");if(t==="light"||t==="dark")document.documentElement.dataset.theme=t}catch(e){}`;

type Theme = "light" | "dark";
type ViewTransitionHandle = { ready: Promise<void>; finished: Promise<void> };
type StartViewTransitionDoc = Document & {
  startViewTransition?: (update: () => void) => ViewTransitionHandle;
};

const listeners = new Set<() => void>();
let transitionSeq = 0;
let fallbackTimer: ReturnType<typeof setTimeout> | undefined;

function isDark() {
  const t = document.documentElement.dataset.theme;
  return t ? t === "dark" : window.matchMedia("(prefers-color-scheme: dark)").matches;
}

function applyTheme(next: Theme) {
  document.documentElement.dataset.theme = next;
  try { localStorage.setItem(KEY, next); } catch {}
  listeners.forEach((sync) => sync());
}

function clearFallbackTimer() {
  if (fallbackTimer !== undefined) {
    clearTimeout(fallbackTimer);
    fallbackTimer = undefined;
  }
}

function radialOrigin(button: HTMLButtonElement | null) {
  const rect = button?.getBoundingClientRect();
  const hasBox = rect && (rect.width > 0 || rect.height > 0);
  const x = hasBox ? rect.left + rect.width / 2 : window.innerWidth / 2;
  const y = hasBox ? rect.top + rect.height / 2 : window.innerHeight / 2;
  const radius = Math.hypot(
    Math.max(x, window.innerWidth - x),
    Math.max(y, window.innerHeight - y),
  );
  return { x, y, radius };
}

/** True when the page is dark, from the toggle or, without a choice, the OS setting. */
export function useDarkTheme() {
  const [dark, setDark] = useState(false);
  useEffect(() => {
    const sync = () => setDark(isDark());
    sync();
    listeners.add(sync);
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener("change", sync);
    const obs = new MutationObserver(sync);
    obs.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    const onStorage = (e: StorageEvent) => {
      if (e.key !== KEY) return;
      if (e.newValue === "light" || e.newValue === "dark") {
        document.documentElement.dataset.theme = e.newValue;
      } else if (e.newValue === null) {
        delete document.documentElement.dataset.theme;
      }
      sync();
    };
    window.addEventListener("storage", onStorage);
    return () => {
      listeners.delete(sync);
      mq.removeEventListener("change", sync);
      obs.disconnect();
      window.removeEventListener("storage", onStorage);
    };
  }, []);
  return dark;
}

export function ThemeToggle() {
  const dark = useDarkTheme();
  const buttonRef = useRef<HTMLButtonElement>(null);

  const toggle = () => {
    const next: Theme = dark ? "light" : "dark";
    const root = document.documentElement;
    const reducedMotion = window.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches ?? false;
    const seq = ++transitionSeq;
    clearFallbackTimer();

    if (reducedMotion) {
      delete root.dataset.themeTransition;
      applyTheme(next);
      return;
    }

    const startViewTransition = (document as StartViewTransitionDoc).startViewTransition;
    if (typeof startViewTransition === "function") {
      const { x, y, radius } = radialOrigin(buttonRef.current);
      root.dataset.themeTransition = "radial";
      let vt: ViewTransitionHandle;
      try {
        vt = startViewTransition.call(document, () => {
          flushSync(() => applyTheme(next));
        });
      } catch {
        if (seq === transitionSeq) delete root.dataset.themeTransition;
        applyTheme(next);
        return;
      }

      vt.ready
        .then(() => {
          if (seq !== transitionSeq || typeof root.animate !== "function") return;
          root.animate(
            { clipPath: [`circle(0px at ${x}px ${y}px)`, `circle(${radius}px at ${x}px ${y}px)`] },
            {
              duration: RADIAL_DURATION_MS,
              easing: "cubic-bezier(0.37, 0, 0.63, 1)",
              fill: "both",
              pseudoElement: "::view-transition-new(root)",
            },
          );
        })
        .catch(() => {});

      vt.finished
        .finally(() => {
          if (seq === transitionSeq) delete root.dataset.themeTransition;
        })
        .catch(() => {});
      return;
    }

    // Fallback for browsers without View Transitions API support.
    root.dataset.themeTransition = "fade";
    applyTheme(next);
    fallbackTimer = setTimeout(() => {
      if (seq === transitionSeq) delete root.dataset.themeTransition;
      fallbackTimer = undefined;
    }, FALLBACK_DURATION_MS);
  };

  return (
    <button
      ref={buttonRef}
      type="button"
      className="icon-btn theme-toggle"
      aria-pressed={dark}
      aria-label="Dark mode"
      title={dark ? "Switch to light mode" : "Switch to dark mode"}
      onClick={toggle}
    >
      <svg width="20" height="20" viewBox="0 0 24 24" aria-hidden="true">
        <g className="theme-sun">
          <circle className="theme-sun-core" cx="12" cy="12" r="4.5" fill="currentColor" />
          <circle className="theme-sun-halo" cx="12" cy="12" r="6.75" fill="none" stroke="currentColor" strokeWidth="1.2" opacity="0.32" />
          <g className="theme-sun-rays" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
            <path d="M12 1.8v2.2M12 20v2.2M1.8 12h2.2M20 12h2.2M5.1 5.1l1.4 1.4M18.9 5.1l-1.4 1.4M18.9 18.9l-1.4-1.4M5.1 18.9l1.4-1.4" />
          </g>
        </g>
        <g className="theme-moon">
          <path className="theme-moon-body" d="M19.4 15.1A8 8 0 1 1 8.9 4.6a7.4 7.4 0 0 0 10.5 10.5Z" fill="currentColor" />
          <path className="theme-moon-star theme-moon-star-lg" d="M17.2 4.2 17.8 5.8 19.4 6.4 17.8 7 17.2 8.6 16.6 7 15 6.4 16.6 5.8Z" fill="currentColor" />
          <path className="theme-moon-star theme-moon-star-sm" d="M19.8 8.9 20.15 9.85 21.1 10.2 20.15 10.55 19.8 11.5 19.45 10.55 18.5 10.2 19.45 9.85Z" fill="currentColor" />
        </g>
      </svg>
    </button>
  );
}
