"use client";

import ElectricLogo from "./ElectricLogo";
import { useDarkTheme } from "./ThemeToggle";

export function BrandMark() {
  const dark = useDarkTheme();
  return (
    <span className="brand-mark-wrap">
      <img className="brand-mark" src="/brand/logo-mark.svg" width={40} height={40} alt="" />
      <ElectricLogo className="brand-electric" src="/brand/logo-mark.svg" color="#cbdbf5" glowColor="#9db8e8" scale={0.5} thickness={1} strands={3} bend={0.25} crackle={0.6} arcs={0.5} flicker={0.25} speed={1.2} cursorRadius={40} theme={dark ? "dark" : "light"} />
    </span>
  );
}
