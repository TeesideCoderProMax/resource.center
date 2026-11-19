const H = {
  blur(tl, s, t) { tl.fromTo(s, { opacity: 0, filter: "blur(8px)" }, { opacity: 1, filter: "blur(0px)", duration: 0.2, ease: "power4.out" }, t); },
  pop(tl, s, t, stagger) { tl.fromTo(s, { opacity: 0, scale: 0.8 }, { opacity: 1, scale: 1, duration: 0.42, ease: "back.out(1.6)", stagger: stagger || 0 }, t); },
  slide(tl, s, t, x, stagger) { tl.fromTo(s, { opacity: 0, x: x === undefined ? -70 : x }, { opacity: 1, x: 0, duration: 0.5, ease: "expo.out", stagger: stagger || 0 }, t); },
  drop(tl, s, t) { tl.fromTo(s, { opacity: 0, y: -60 }, { opacity: 1, y: 0, duration: 0.5, ease: "expo.out" }, t); },
  stamp(tl, s, t) { tl.fromTo(s, { opacity: 0, scale: 1.7 }, { opacity: 1, scale: 1, duration: 0.24, ease: "power4.in" }, t); },
  draw(tl, s, t, d) { tl.fromTo(s, { strokeDashoffset: 1, strokeDasharray: 1 }, { strokeDashoffset: 0, duration: d || 0.5, ease: "power2.inOut" }, t); },
  out(tl, s, t) { tl.to(s, { opacity: 0, filter: "blur(8px)", duration: 0.2, ease: "power2.in" }, t); },
  shake(tl, s, t) { tl.fromTo(s, { x: -16 }, { x: 0, duration: 0.45, ease: "elastic.out(1.4, 0.25)", immediateRender: false }, t); },
  whipIn(tl) { tl.fromTo("#stage", { x: 1920 }, { x: 0, duration: 0.5, ease: "expo.out" }, 0); },
  whipOut(tl, D) { tl.to("#stage", { x: -1920, duration: 0.32, ease: "power3.in" }, D - 0.34); },
  head(tl, t) { H.blur(tl, ".title", t); H.blur(tl, ".sub", t + 0.15); },
};
