#!/usr/bin/env python3
"""Render each script line with Kokoro, stitch narration.wav and write timing.json."""
import json, os, subprocess, sys, wave

GAP, SCENE_GAP, LEAD = 0.22, 0.55, 0.3
py = sys.argv[1]
script = json.load(open("script.json"))
os.makedirs("assets/voice/lines", exist_ok=True)
env = dict(os.environ, HYPERFRAMES_PYTHON=py)

def duration(path):
    with wave.open(path) as w:
        return w.getnframes() / w.getframerate()

t, timing, parts = LEAD, {"scenes": {}, "lines": {}}, [("sil", LEAD)]
for scene, lines in script.items():
    start = t
    for line in lines:
        id_, caption, speech = line[0], line[1], line[2] if len(line) > 2 else line[1]
        path = f"assets/voice/lines/{id_}.wav"
        if not os.path.exists(path):
            subprocess.run(["npx", "hyperframes", "tts", speech, "-v", "af_heart", "-s", "1.2", "-o", path, "--json"],
                           env=env, check=True, stdout=subprocess.DEVNULL)
        d = duration(path)
        timing["lines"][id_] = {"start": round(t, 3), "end": round(t + d, 3), "rel": round(t - start, 3), "text": caption, "scene": scene}
        parts += [("wav", path), ("sil", GAP)]
        t += d + GAP
    parts[-1] = ("sil", SCENE_GAP)
    t += SCENE_GAP - GAP
    timing["scenes"][scene] = {"start": round(start - (LEAD if scene == "hook" else 0), 3), "end": round(t, 3)}
timing["duration"] = round(t + 2, 3)

with wave.open(parts[1][1]) as w:
    rate, width, ch = w.getframerate(), w.getsampwidth(), w.getnchannels()
with wave.open("assets/voice/narration.wav", "wb") as out:
    out.setnchannels(ch); out.setsampwidth(width); out.setframerate(rate)
    for kind, value in parts:
        if kind == "sil":
            out.writeframes(b"\0" * int(value * rate) * width * ch)
        else:
            with wave.open(value) as w:
                out.writeframes(w.readframes(w.getnframes()))
json.dump(timing, open("timing.json", "w"), indent=1)
print(timing["duration"], {k: round(v["end"] - v["start"], 1) for k, v in timing["scenes"].items()})
