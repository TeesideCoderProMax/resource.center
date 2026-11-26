import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ThemeToggle } from "./ThemeToggle";

function stubMatchMedia(reducedMotion = false) {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: query.includes("reduced-motion") ? reducedMotion : false,
    addEventListener() {},
    removeEventListener() {},
  }));
}

beforeEach(() => {
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.themeTransition;
  localStorage.clear();
  stubMatchMedia(false);
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  Reflect.deleteProperty(document, "startViewTransition");
  Reflect.deleteProperty(document.documentElement, "animate");
});

it("switches between light and dark with fallback fade when view transitions are absent", async () => {
  vi.useFakeTimers();
  render(<ThemeToggle />);
  const button = screen.getByRole("button", { name: "Dark mode" });
  expect(button).toHaveAttribute("aria-pressed", "false");

  fireEvent.click(button);
  expect(document.documentElement.dataset.theme).toBe("dark");
  expect(document.documentElement.dataset.themeTransition).toBe("fade");
  expect(localStorage.getItem("theme")).toBe("dark");
  expect(button).toHaveAttribute("aria-pressed", "true");

  vi.advanceTimersByTime(749);
  expect(document.documentElement.dataset.themeTransition).toBe("fade");

  vi.advanceTimersByTime(1);
  expect(document.documentElement.dataset.themeTransition).toBeUndefined();

  fireEvent.click(button);
  expect(document.documentElement.dataset.theme).toBe("light");
  expect(button).toHaveAttribute("aria-pressed", "false");
});

it("uses a radial view transition from the toggle button and cleans up after finishing", async () => {
  let resolveReady!: () => void;
  let resolveFinished!: () => void;
  const startViewTransition = vi.fn((update: () => void) => {
    update();
    return {
      ready: new Promise<void>((resolve) => { resolveReady = resolve; }),
      finished: new Promise<void>((resolve) => { resolveFinished = resolve; }),
    };
  });
  const animate = vi.fn();
  Object.defineProperty(document, "startViewTransition", { configurable: true, value: startViewTransition });
  Object.defineProperty(document.documentElement, "animate", { configurable: true, value: animate });

  render(<ThemeToggle />);
  const button = screen.getByRole("button", { name: "Dark mode" });
  vi.spyOn(button, "getBoundingClientRect").mockReturnValue({
    x: 900, y: 12, width: 38, height: 38, top: 12, left: 900, right: 938, bottom: 50, toJSON: () => ({}),
  });

  fireEvent.click(button);

  expect(startViewTransition).toHaveBeenCalledTimes(1);
  expect(document.documentElement.dataset.theme).toBe("dark");
  expect(button).toHaveAttribute("aria-pressed", "true");
  expect(document.documentElement.dataset.themeTransition).toBe("radial");

  resolveReady();
  await waitFor(() => expect(animate).toHaveBeenCalledTimes(1));
  expect(animate).toHaveBeenCalledWith(
    { clipPath: [expect.stringMatching(/^circle\(0px at 919px 31px\)$/), expect.stringMatching(/^circle\(.+px at 919px 31px\)$/)] },
    expect.objectContaining({
      pseudoElement: "::view-transition-new(root)",
      duration: 850,
      easing: "cubic-bezier(0.37, 0, 0.63, 1)",
    }),
  );

  resolveFinished();
  await waitFor(() => expect(document.documentElement.dataset.themeTransition).toBeUndefined());
});

it("keeps newer transition active when toggled rapidly before the previous transition finishes", async () => {
  const handles: Array<{ rejectReady: () => void; resolveFinished: () => void }> = [];
  const startViewTransition = vi.fn((update: () => void) => {
    update();
    let rejectReady!: () => void;
    let resolveFinished!: () => void;
    const ready = new Promise<void>((_, reject) => { rejectReady = reject; });
    const finished = new Promise<void>((resolve) => { resolveFinished = resolve; });
    handles.push({ rejectReady, resolveFinished });
    return { ready, finished };
  });
  Object.defineProperty(document, "startViewTransition", { configurable: true, value: startViewTransition });
  Object.defineProperty(document.documentElement, "animate", { configurable: true, value: vi.fn() });

  render(<ThemeToggle />);
  const button = screen.getByRole("button", { name: "Dark mode" });

  fireEvent.click(button);
  fireEvent.click(button);

  handles[0].rejectReady();
  handles[0].resolveFinished();
  await Promise.resolve();
  expect(document.documentElement.dataset.themeTransition).toBe("radial");
  expect(document.documentElement.dataset.theme).toBe("light");

  handles[1].resolveFinished();
  await waitFor(() => expect(document.documentElement.dataset.themeTransition).toBeUndefined());
});

it("skips animations when prefers-reduced-motion is enabled", () => {
  stubMatchMedia(true);
  const startViewTransition = vi.fn();
  Object.defineProperty(document, "startViewTransition", { configurable: true, value: startViewTransition });

  render(<ThemeToggle />);
  const button = screen.getByRole("button", { name: "Dark mode" });
  fireEvent.click(button);

  expect(startViewTransition).not.toHaveBeenCalled();
  expect(document.documentElement.dataset.theme).toBe("dark");
  expect(document.documentElement.dataset.themeTransition).toBeUndefined();
});

it("renders the two-layer sun and crescent-with-stars icon without legacy moon-mask", () => {
  const { container } = render(<ThemeToggle />);
  expect(container.querySelector(".theme-sun")).toBeInTheDocument();
  expect(container.querySelector(".theme-moon")).toBeInTheDocument();
  expect(container.querySelectorAll(".theme-moon-star")).toHaveLength(2);
  expect(container.querySelector("#moon-mask")).toBeNull();
});
