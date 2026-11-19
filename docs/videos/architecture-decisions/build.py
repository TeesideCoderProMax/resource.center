#!/usr/bin/env python3
"""Assemble index.html and compositions/ from src/ templates and timing.json."""
import json, os, re

t = json.load(open("timing.json"))
css, js = open("src/shared.css").read(), open("src/shared.js").read()
order = list(t["scenes"])
os.makedirs("compositions", exist_ok=True)
DURATION = t["duration"]

def scene_file(sid, inner):
    style = "\n".join(re.findall(r"<style>(.*?)</style>", inner, re.S))
    script = "\n".join(re.findall(r"<script>(.*?)</script>", inner, re.S))
    body = re.sub(r"<style>.*?</style>|<script>.*?</script>", "", inner, flags=re.S)
    return style, body, script

for sid in order:
    sc = t["scenes"][sid]
    D = round(sc["end"] - sc["start"], 3)
    offset = sc["start"]
    T = {k: round(v["start"] - offset, 3) for k, v in t["lines"].items() if v["scene"] == sid}
    style, body, script = scene_file(sid, open(f"src/{sid}.html").read())
    body = re.sub(r'id="(b1)">([^<]+)<', lambda m: f'id="{m[1]}">' + " ".join(f'<span class="w">{w}</span>' for w in m[2].split()) + "<", body) if sid == "close" else body
    open(f"compositions/{sid}.html", "w").write(f"""<template>
<style>{css}{style}</style>
<div id="root" data-composition-id="{sid}" data-width="1920" data-height="1080" data-layout-allow-overlap>
{body}
</div>
<script>
{js}
const T = {json.dumps(T)}, D = {D};
const tl = gsap.timeline({{ paused: true }});
{script}
tl.set({{}}, {{}}, D - 0.01);
window.__timelines["{sid}"] = tl;
</script>
</template>
""")

# Captions: groups of up to six words, timed by character share inside each voiced line.
groups = []
for lid, line in t["lines"].items():
    words = line["text"].split()
    chunks, cur = [], []
    for w in words:
        cur.append(w)
        if len(cur) >= 6 or (w[-1] in ",.:;?" and len(cur) >= 3):
            chunks.append(cur); cur = []
    if cur:
        chunks.append(cur)
    if len(chunks) > 1 and len(chunks[-1]) <= 2:
        tail = chunks.pop(-2) + chunks.pop()
        cut = len(tail) if len(tail) <= 7 else len(tail) // 2
        chunks += [tail[:cut]] + ([tail[cut:]] if tail[cut:] else [])
    total = sum(len(" ".join(c)) + 1 for c in chunks)
    at, span = line["start"], line["end"] - line["start"]
    for i, c in enumerate(chunks):
        d = span * (len(" ".join(c)) + 1) / total
        groups.append((f"{lid}c{i}", " ".join(c), round(at, 3), round(d, 3)))
        at += d
cap = "\n".join(
    f'<div id="{i}" class="cap clip" data-start="{s}" data-duration="{d}" data-track-index="1"><span>{x}</span></div>'
    for i, x, s, d in groups)
open("compositions/captions.html", "w").write(f"""<template>
<style>{css}
#root {{ pointer-events: none; }}
.cap {{ position: absolute; left: 0; width: 1920px; bottom: 44px; text-align: center; }}
.cap span {{ display: inline-block; background: #0b1c30; color: #fff; font-size: 38px; font-weight: 600; padding: 10px 26px; border-radius: 10px; }}
</style>
<div id="root" data-composition-id="captions" data-width="1920" data-height="1080" data-layout-allow-overflow>
{cap}
</div>
<script>
const tl = gsap.timeline({{ paused: true }});
tl.set({{}}, {{}}, {DURATION - 0.01});
window.__timelines["captions"] = tl;
</script>
</template>
""")

# Route strip plus the part cards that pull down from it.
names = ["Ingress", "Containers", "Outbox", "Ports", "Publisher", "Services", "Retries", "Leases", "Recovery"]
sids = ["ingress", "apps", "outbox", "ports", "publisher", "services", "dupes", "lease", "recovery"]
A = [t["scenes"][s]["start"] for s in sids]
step = 1728 / 9
stops = "\n".join(
    f'<div class="stop" id="st{i}" style="left:{96 + step * i:.0f}px"><i></i>{n}</div>' for i, n in enumerate(names))
cards = [("ingress", "Get it running", "Without managing servers"),
         ("outbox", "Don’t lose it", "One change, one saved request"),
         ("dupes", "Survive the retries", "When requests overlap or services fail")]
card_html = "\n".join(
    f'<div class="card" id="k{s}"><div>{a}</div><p>{b}</p></div>' for s, a, b in cards)
card_js = "\n".join(
    f'at={t["scenes"][s]["start"]};'
    f'tl.fromTo("#shade",{{clipPath:"inset(0 0 100% 0)"}},{{clipPath:"inset(0 0 0% 0)",duration:.42,ease:"power2.inOut"}},at-.42);'
    f'tl.fromTo("#k{s}",{{opacity:0}},{{opacity:1,duration:.01}},at-.3);'
    f'tl.fromTo("#k{s} div",{{y:60,filter:"blur(8px)"}},{{y:0,filter:"blur(0px)",duration:.45,ease:"expo.out"}},at-.25);'
    f'tl.to("#shade",{{clipPath:"inset(0 0 100% 0)",duration:.4,ease:"power2.inOut"}},at+1.6);'
    f'tl.set("#k{s}",{{opacity:0}},at+2.02);'
    for s, _, _ in cards)
open("compositions/route.html", "w").write(f"""<template>
<style>{css}
#root {{ pointer-events: none; }}
#strip {{ position: absolute; left: 0; top: 0; width: 1920px; height: 96px; background: #0b1c30; color: #fff; }}
.stop {{ position: absolute; top: 20px; width: 190px; display: flex; align-items: center; gap: 10px; font-size: 28px; font-weight: 700; opacity: .5; white-space: nowrap; }}
.stop i {{ display: block; width: 18px; height: 18px; flex: none; border-radius: 50%; background: #fff; }}
#shade {{ position: absolute; inset: 0; background: #0b1c30; color: #fff; }}
.card {{ position: absolute; left: 96px; top: 390px; opacity: 0; width: 1728px; }}
.card div {{ font-size: 130px; font-weight: 800; letter-spacing: -.025em; }}
.card p {{ font-size: 46px; font-weight: 500; margin-top: 16px; color: #a7f3d0; }}
</style>
<div id="root" data-composition-id="route" data-width="1920" data-height="1080" data-layout-allow-overlap data-layout-allow-occlusion>
<div id="shade">{card_html}</div>
<div id="strip">{stops}</div>
</div>
<script>
const tl = gsap.timeline({{ paused: true }});
const A = {json.dumps(A)};
let at = 0;
tl.set("#shade", {{ clipPath: "inset(0 0 100% 0)" }}, 0);
tl.fromTo("#strip", {{ y: -96 }}, {{ y: 0, duration: .4, ease: "power2.inOut" }}, A[0] - .42);
A.forEach((a, i) => {{
  tl.fromTo("#st" + i, {{ opacity: .5 }}, {{ opacity: 1, duration: .2 }}, a);
  tl.fromTo("#st" + i + " i", {{ backgroundColor: "#ffffff" }}, {{ backgroundColor: "#a7f3d0", scale: 1.3, duration: .2 }}, a);
}});
{card_js}
tl.to("#strip", {{ y: -96, duration: .3, ease: "power2.in" }}, {t["scenes"]["close"]["start"] - .3});
tl.set({{}}, {{}}, {DURATION - 0.01});
window.__timelines["route"] = tl;
</script>
</template>
""")

hosts = []
for i, sid in enumerate(order):
    sc = t["scenes"][sid]
    end = DURATION if sid == "close" else sc["end"]
    hosts.append(f'<div id="{sid}" data-composition-id="{sid}" data-composition-src="compositions/{sid}.html" '
                 f'data-start="{sc["start"]}" data-duration="{round(end - sc["start"], 3)}" data-track-index="1" '
                 f'data-track-kind="graphics" data-width="1920" data-height="1080"></div>')
hosts_html = "\n      ".join(hosts)
open("index.html", "w").write(f"""<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=1920, height=1080" />
    <script src="https://cdn.jsdelivr.net/npm/gsap@3.15.0/dist/gsap.min.js"></script>
    <style>
      * {{ margin: 0; padding: 0; box-sizing: border-box; }}
      html, body {{ margin: 0; width: 1920px; height: 1080px; overflow: hidden; background: #7c3aed; }}
      #root {{ width: 100%; height: 100%; }}
      #route {{ z-index: 20; }}
      #captions {{ z-index: 30; }}
    </style>
  </head>
  <body>
    <div id="root" data-composition-id="main" data-start="0" data-duration="{DURATION}" data-width="1920" data-height="1080">
      {hosts_html}
      <div id="route" data-composition-id="route" data-composition-src="compositions/route.html" data-start="0" data-duration="{DURATION}" data-track-index="2" data-track-kind="graphics" data-width="1920" data-height="1080"></div>
      <div id="captions" data-composition-id="captions" data-composition-src="compositions/captions.html" data-start="0" data-duration="{DURATION}" data-track-index="3" data-track-kind="captions" data-width="1920" data-height="1080"></div>
      <audio id="narration" class="clip" src="assets/voice/narration.wav" data-start="0" data-track-index="4" data-volume="1"></audio>
    </div>
    <script>
      window.__timelines = window.__timelines || {{}};
      window.__timelines["main"] = gsap.timeline({{ paused: true }});
    </script>
  </body>
</html>
""")
print("built", len(groups), "caption groups", DURATION)
