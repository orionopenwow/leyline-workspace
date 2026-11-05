#!/usr/bin/env python3
"""The reel kit's edit: recorded takes + a small script (scripts/<id>.json) in, finished videos out.

    python3 -I tools/reels/edit.py [reel ...] [--takes DIR] [--out DIR] [--formats reel,wide] [--placeholders]
                                   [--final] [--fast] [--list] [--check]

For each script: the 1 s intro sting, the title card, each shot (a piece of a take, sped up or slowed, fitted to the
frame, its lower-third caption fading in and out), the end card ("leylinelauncher.com"), joined with short cross-fades;
an MP4 per format (H.264, 30 fps, a silent audio track so every app takes it), an .srt of the captions, a GIF for
Reddit from the shots the script names, and a contact sheet (a frame every 2 s) to look over. Out:
tools/reels/out/<id>/<id>-<format>.mp4, <id>.gif, <id>-<format>.srt, <id>-<format>-sheet.jpg (git-ignored).

Takes (scripts/takes.json says what each one is and how it is recorded) are looked up in --takes (default
tools/reels/takes, git-ignored) as <take>.mp4 with <take>.json beside it (its length and named moments, "marks", from
record.py), or <take>/frames.json (a launcher take from takes.cjs: screencast frames with their times, made into
<take>.mp4 here the first time). A shot's "in" and "out" are seconds, seconds from the end (negative), or
[mark, offset] ("in": ["carry", -1] is one second before the take's "carry" moment).

The launch video (scripts/launch-v2.json, "parts") is cut from the other scripts' shots, so re-running this after new
takes or a new reel brings it up to date by itself. A script still holding TODO(owner) words is a draft: its videos
wear a DRAFT tag until --final, and --final refuses it (owner's words first). A shot whose take isn't recorded yet
is left out (the script's note says so) unless --placeholders, which shows a card in its place: a preview of the cut,
never something to post.

Needs ffmpeg and Pillow. Nothing here uploads or posts anything.
"""
import argparse
import functools
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import brand as B  # noqa: E402
import cards  # noqa: E402

SCRIPTS = os.path.join(HERE, "scripts")
TAKES = os.path.join(HERE, "takes")
OUT = os.path.join(HERE, "out")
TODO = "TODO(owner)"
SOURCES = ("launcher-fake", "launcher", "site", "phone", "game", "hand")
FITS = ("contain", "cover")
# a version number inside a sentence ("your 3.3.5a folder"); the launcher tests' rule (launcher/tests/firstrun)
VERSION_PROSE = re.compile(r"[A-Za-z,][ \t]+v?\d+\.\d+(\.\d+)?[a-z]?[ \t]+[a-z]")


class ScriptError(ValueError):
    pass


# ---------------------------------------------------------------- scripts
def load(path):
    """A script: JSON, or YAML when PyYAML is there (the shipped ones are JSON, so the PC needs nothing extra)."""
    with open(path, encoding="utf-8") as f:
        text = f.read()
    if path.endswith((".yaml", ".yml")):
        import yaml  # noqa: PLC0415 - only for a YAML script
        return yaml.safe_load(text)
    return json.loads(text)


def catalog(scripts_dir=SCRIPTS):
    return load(os.path.join(scripts_dir, "takes.json"))["takes"]


def all_scripts(scripts_dir=SCRIPTS):
    out = {}
    for name in sorted(os.listdir(scripts_dir)):
        if name == "takes.json" or not name.endswith((".json", ".yaml", ".yml")):
            continue
        s = load(os.path.join(scripts_dir, name))
        out[s["id"]] = s
    return out


def words(script):
    """Every piece of text a script puts on screen (title, kicker, sub, captions, the end line)."""
    out = [script.get(k, "") for k in ("title", "kicker", "sub", "end_line")]
    for sh in script.get("shots", []):
        out += [sh.get("caption", ""), sh.get("sub", "")]
    return [w for w in out if w]


def shown(text):
    """What a text shows on screen: a TODO(owner) draft shows its draft (after the colon)."""
    t = str(text)
    if t.startswith(TODO):
        t = t[len(TODO):].lstrip(" :")
    return t


def is_draft(script, scripts=None):
    if any(str(w).startswith(TODO) for w in words(script)):
        return True
    if scripts and script.get("parts"):
        return any(is_draft(scripts[p["reel"]]) for p in script["parts"] if p.get("reel") in scripts)
    return False


def text_problems(text):
    """The standing rules for words players see: no brand names, no version in a sentence, Log in not Sign in."""
    t = shown(text)
    low = t.lower()
    out = []
    for b in B.BRANDS:
        if re.search(r"\b%s\b" % re.escape(b), low):
            out.append("a brand word (%s): %s" % (b, t))
    if VERSION_PROSE.search(t):
        out.append("a version number in a sentence: " + t)
    if re.search(r"\bsign(ed)?[ -]?in\b", low):
        out.append('"Log in", never "Sign in": ' + t)
    return out


def validate(script, takes, scripts=None):
    """Problems with one script (a list of strings; empty is fine)."""
    p = []
    sid = script.get("id", "?")
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]{1,40}", str(script.get("id", ""))):
        p.append("%s: id must be lowercase words with dashes" % sid)
    if script.get("kind") not in ("reel", "flow", "launch"):
        p.append("%s: kind is reel, flow or launch" % sid)
    if not script.get("title"):
        p.append("%s: no title" % sid)
    for f in script.get("formats", []):
        if f not in B.FORMATS:
            p.append("%s: unknown format %s" % (sid, f))
    for w in words(script):
        p += ["%s: %s" % (sid, x) for x in text_problems(w)]
    if script.get("kind") == "launch":
        if not script.get("parts"):
            p.append("%s: a launch video needs parts" % sid)
        for part in script.get("parts", []):
            other = (scripts or {}).get(part.get("reel"))
            if scripts is not None and other is None:
                p.append("%s: part names an unknown reel %s" % (sid, part.get("reel")))
                continue
            if other is not None:
                for i in part.get("shots", []):
                    if not isinstance(i, int) or not 0 <= i < len(other.get("shots", [])):
                        p.append("%s: %s has no shot %s" % (sid, part["reel"], i))
        return p
    if not script.get("shots"):
        p.append("%s: no shots" % sid)
    for i, sh in enumerate(script.get("shots", [])):
        where = "%s shot %d" % (sid, i)
        if sh.get("take") not in takes:
            p.append("%s: take %s isn't in takes.json" % (where, sh.get("take")))
        for k in ("in", "out"):
            v = sh.get(k)
            if v is None or not (isinstance(v, (int, float)) or (isinstance(v, list) and len(v) == 2 and isinstance(v[0], str))):
                p.append("%s: %s is seconds or [mark, offset]" % (where, k))
            elif isinstance(v, list) and sh.get("take") in takes and v[0] not in takes[sh["take"]].get("marks", []):
                p.append("%s: take %s has no mark %s (takes.json marks)" % (where, sh["take"], v[0]))
        if not 0.25 <= float(sh.get("speed", 1)) <= 4:
            p.append("%s: speed between 0.25 and 4" % where)
        if any(f not in FITS for f in (sh["fit"].values() if isinstance(sh.get("fit"), dict) else [sh.get("fit", "contain")])):
            p.append("%s: fit is contain or cover" % where)
    g = script.get("gif")
    if g:
        for i in g.get("shots", []):
            if not isinstance(i, int) or not 0 <= i < len(script.get("shots", [])):
                p.append("%s: the GIF names shot %s" % (sid, i))
    return p


# ---------------------------------------------------------------- takes
def tool(name):
    """ffmpeg or ffprobe: on PATH, or where winget put it (winget adds it to PATH only for consoles opened later)."""
    f = shutil.which(name)
    if f:
        return f
    base = os.path.join(os.environ.get("LOCALAPPDATA", ""), "Microsoft", "WinGet", "Packages")
    if os.path.isdir(base):
        for d in sorted(os.listdir(base)):
            if d.startswith("Gyan.FFmpeg"):
                for top, _dirs, files in os.walk(os.path.join(base, d)):
                    if name + ".exe" in files:
                        return os.path.join(top, name + ".exe")
    return None


def ffprobe_seconds(path):
    r = subprocess.run([tool("ffprobe") or "ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1", path],
                       capture_output=True, text=True)
    try:
        return float(r.stdout.strip())
    except ValueError:
        return 0.0


def frames_to_mp4(folder, out, fps=B.FPS):
    """A takes.cjs folder (frames.json and its JPEGs, times in slow motion) as a steady 30 fps MP4, and its marks."""
    with open(os.path.join(folder, "frames.json"), encoding="utf-8") as f:
        j = json.load(f)
    slow = float(j.get("slow", 1)) or 1.0
    t0, t1 = j["t0"], j["t1"]
    frames = [x for x in j["frames"] if os.path.isfile(os.path.join(folder, x["file"]))]
    if not frames:
        raise ScriptError("no frames in " + folder)
    lst = os.path.join(folder, "concat.txt")
    with open(lst, "w", encoding="utf-8") as f:
        f.write("ffconcat version 1.0\n")
        for i, fr in enumerate(frames):
            nxt = frames[i + 1]["t"] if i + 1 < len(frames) else t1
            dur = max(1.0 / fps, (nxt - max(fr["t"], t0)) / slow)
            f.write("file '%s'\nduration %.4f\n" % (fr["file"], dur))
        f.write("file '%s'\n" % frames[-1]["file"])
    run(["ffmpeg", "-y", "-v", "error", "-f", "concat", "-safe", "0", "-i", lst, "-vf",
         "fps=%d,scale=trunc(iw/2)*2:trunc(ih/2)*2,format=yuv420p" % fps, "-c:v", "libx264", "-preset", "veryfast", "-crf", "16", out])
    marks = {m["label"]: (m["t"] - t0) / slow for m in j.get("marks", [])}
    meta = {"seconds": (t1 - t0) / slow, "marks": marks, "source": "launcher-fake"}
    with open(os.path.splitext(out)[0] + ".json", "w", encoding="utf-8") as f:
        json.dump(meta, f, indent=1)
    return meta


def find_take(takes_dir, take, fmt=None):
    """(mp4 path, meta) for a recorded take, or (None, None). A take may have a version made for one format
    (<take>@reel.mp4: the launcher recorded tall for the 9:16 reel); that one is used for that format."""
    for name in ([take + "@" + fmt] if fmt else []) + [take]:
        mp4 = os.path.join(takes_dir, name + ".mp4")
        frames = os.path.join(takes_dir, name, "frames.json")
        if os.path.isfile(frames) and (not os.path.isfile(mp4) or os.path.getmtime(frames) > os.path.getmtime(mp4)):
            frames_to_mp4(os.path.dirname(frames), mp4)
        if not os.path.isfile(mp4):
            continue
        meta = {}
        try:
            with open(os.path.join(takes_dir, name + ".json"), encoding="utf-8") as f:
                meta = json.load(f)
        except (OSError, ValueError):
            pass
        if not meta.get("seconds"):
            meta["seconds"] = ffprobe_seconds(mp4)
        meta.setdefault("marks", {})
        meta["variant"] = name
        return mp4, meta
    return None, None


def per(shot, key, fmt, default):
    """A shot option, the same for every format or one per format ({"reel": ..., "wide": ...})."""
    v = shot.get(key, default)
    if isinstance(v, dict):
        return v.get(fmt, default)
    return v


def frame_size(path):
    r = subprocess.run([tool("ffprobe") or "ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height", "-of", "csv=p=0", path],
                       capture_output=True, text=True)
    try:
        w, h = (int(x) for x in r.stdout.strip().split(",")[:2])
        return w, h
    except ValueError:
        return 0, 0


def at(ref, meta):
    """A shot's in or out as seconds into its take."""
    secs = float(meta.get("seconds") or 0)
    if isinstance(ref, list):
        label, off = ref
        if label not in meta.get("marks", {}):
            raise ScriptError("the take has no mark %r (it has %s)" % (label, ", ".join(sorted(meta.get("marks", {}))) or "none"))
        v = float(meta["marks"][label]) + float(off)
    else:
        v = float(ref)
        if v < 0:
            v = secs + v
    return max(0.0, min(secs, v)) if secs else max(0.0, v)


# ---------------------------------------------------------------- ffmpeg
def run(cmd):
    cmd = [tool(cmd[0]) or cmd[0]] + list(cmd[1:])
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError("ffmpeg failed (%d): %s\n%s" % (r.returncode, " ".join(cmd[:6]) + " ...", r.stderr[-1500:]))
    return r


class Enc:
    def __init__(self, fast=False):
        self.args = ["-c:v", "libx264", "-preset", "ultrafast" if fast else "medium", "-crf", "20" if fast else "17",
                     "-pix_fmt", "yuv420p", "-r", str(B.FPS)]


def still(img, seconds, out, enc, zoom=0.0):
    """A card as a clip; zoom > 0 is a slow push in (0.03 = 3% over the card)."""
    w, h = img.size
    png = out + ".png"
    img.save(png)
    vf = "format=yuv420p"
    if zoom:
        n = int(seconds * B.FPS)
        big = "scale=%d:%d" % (w * 2, h * 2)
        vf = "%s,zoompan=z='1+%f*on/%d':x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':d=%d:s=%dx%d:fps=%d,format=yuv420p" % (big, zoom, n, n, w, h, B.FPS)
    run(["ffmpeg", "-y", "-v", "error", "-loop", "1", "-framerate", str(B.FPS), "-t", "%.3f" % seconds, "-i", png, "-vf", vf] + enc.args + ["-t", "%.3f" % seconds, out])
    os.remove(png)
    return seconds


def frames_clip(frames, out, enc):
    w, h = frames[0].size
    p = subprocess.Popen([tool("ffmpeg") or "ffmpeg", "-y", "-v", "error", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", "%dx%d" % (w, h), "-r", str(B.FPS),
                          "-i", "-"] + enc.args + [out], stdin=subprocess.PIPE, stderr=subprocess.PIPE)
    for fr in frames:
        p.stdin.write(fr.tobytes())
    p.stdin.close()
    err = p.stderr.read().decode("utf-8", "replace")
    p.stderr.close()
    if p.wait() != 0:
        raise RuntimeError("ffmpeg (sting) failed: " + err[-800:])
    return len(frames) / B.FPS


def shot_clip(src, t_in, t_out, fmt, shot, out, work, enc):
    """One shot: the take's piece, at its speed, fitted to the frame over a blurred, darkened copy of itself, with its
    lower third fading in and out. Returns its length in seconds."""
    W, H = B.FORMATS[fmt]
    st = B.STAGE[fmt]
    SX, SY, SW, SH = int(st[0] * W), int(st[1] * H), int(st[2] * W) // 2 * 2, int(st[3] * H) // 2 * 2
    speed = float(shot.get("speed", 1))
    dur = max(0.3, (t_out - t_in) / speed)
    sw, sh_ = frame_size(src)
    # a wide take in the tall reel: bigger than "fits", its sides cut around the focus (a 16:9 picture whole in a 9:16
    # frame is a strip); a take recorded for this format fits as it is
    auto = 1.6 if (H > W and sw > sh_) else 1.0
    zoom = float(per(shot, "zoom", fmt, auto))
    fx, fy = per(shot, "focus", fmt, [0.5, 0.5])
    fit = per(shot, "fit", fmt, "contain")
    grow = "increase" if fit == "cover" else "decrease"
    fg = "scale=%d:%d:force_original_aspect_ratio=%s" % (SW * zoom, SH * zoom, grow)
    # the zoomed picture cut to the stage around the focus point (a crop no bigger than the picture or the stage)
    crop = "crop=w='min(iw,%d)':h='min(ih,%d)':x='(iw-ow)*%f':y='(ih-oh)*%f'" % (SW, SH, fx, fy)
    vf = ("[0:v]trim=start=%.3f:end=%.3f,setpts=(PTS-STARTPTS)/%f,fps=%d,split[a][b];"
          "[a]scale=%d:%d:force_original_aspect_ratio=increase,crop=%d:%d,boxblur=28:2,eq=brightness=-0.18:saturation=0.8[bg];"
          "[b]%s,%s,scale=trunc(iw/2)*2:trunc(ih/2)*2[fg];"
          "[bg][fg]overlay=x=%d+(%d-w)/2:y=%d+(%d-h)/2,setsar=1[v0]") % (t_in, t_out, speed, B.FPS, W, H, W, H, fg, crop, SX, SW, SY, SH)
    inputs = ["-i", src]
    last = "v0"
    if shot.get("caption"):
        lt = os.path.join(work, os.path.basename(out) + ".lt.png")
        cards.lower_third(fmt, shown(shot["caption"]), shown(shot.get("sub", ""))).save(lt)
        inputs += ["-loop", "1", "-framerate", str(B.FPS), "-t", "%.3f" % dur, "-i", lt]
        fin, fout = min(0.35, dur / 4), max(0.0, dur - 0.45)
        vf += (";[1:v]format=rgba,fade=in:st=%.2f:d=0.3:alpha=1,fade=out:st=%.2f:d=0.3:alpha=1[lt];"
               "[%s][lt]overlay=0:0:shortest=1[v1]") % (fin, fout, last)
        last = "v1"
    vf += ";[%s]format=yuv420p[out]" % last
    run(["ffmpeg", "-y", "-v", "error"] + inputs + ["-filter_complex", vf, "-map", "[out]", "-t", "%.3f" % dur] + enc.args + [out])
    return dur


def join(parts, out, enc, badge=None, fade=B.XFADE):
    """The parts (clip, seconds) joined with cross-fades, the DRAFT tag over all if given, a silent stereo track."""
    inputs, fl = [], []
    for i, (clip, _) in enumerate(parts):
        inputs += ["-i", clip]
        fl.append("[%d:v]settb=AVTB,fps=%d,format=yuv420p,setsar=1[p%d]" % (i, B.FPS, i))
    cur, total = "p0", parts[0][1]
    for i in range(1, len(parts)):
        nxt = "x%d" % i
        fl.append("[%s][p%d]xfade=transition=fade:duration=%.3f:offset=%.3f[%s]" % (cur, i, fade, total - fade, nxt))
        total = total + parts[i][1] - fade
        cur = nxt
    n = len(parts)
    if badge:
        inputs += ["-loop", "1", "-framerate", str(B.FPS), "-t", "%.3f" % total, "-i", badge]
        fl.append("[%s][%d:v]overlay=0:0:shortest=1[bd]" % (cur, n))
        cur = "bd"
        n += 1
    inputs += ["-f", "lavfi", "-t", "%.3f" % total, "-i", "anullsrc=r=48000:cl=stereo"]
    run(["ffmpeg", "-y", "-v", "error"] + inputs + ["-filter_complex", ";".join(fl), "-map", "[%s]" % cur, "-map", "%d:a" % n,
         "-t", "%.3f" % total] + enc.args + ["-c:a", "aac", "-b:a", "96k", "-movflags", "+faststart", out])
    return total


def gif(src, start, seconds, out, width=640, fps=12):
    """A looping GIF for Reddit (palette from the clip itself, a light dither): small enough to post as it is."""
    vf = ("fps=%d,scale=%d:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=160:stats_mode=diff[p];"
          "[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle") % (fps, width)
    run(["ffmpeg", "-y", "-v", "error", "-ss", "%.3f" % start, "-t", "%.3f" % seconds, "-i", src, "-vf", vf, "-loop", "0", out])


def sheet(src, out, every=2.0):
    """A contact sheet: one frame every `every` seconds, small, in a grid (to look over a cut without playing it)."""
    secs = ffprobe_seconds(src)
    n = max(1, int(secs / every))
    cols = 6
    rows = max(1, -(-n // cols))
    run(["ffmpeg", "-y", "-v", "error", "-i", src, "-vf", "fps=1/%g,scale=320:-2,tile=%dx%d:padding=4:color=0x12151b" % (every, cols, rows),
         "-frames:v", "1", "-q:v", "4", out])


def srt_time(t):
    ms = int(round(t * 1000))
    return "%02d:%02d:%02d,%03d" % (ms // 3600000, ms // 60000 % 60, ms // 1000 % 60, ms % 1000)


# ---------------------------------------------------------------- one script
def shots_of(script, scripts):
    """The shots to cut: a reel's own, or a launch video's parts from the other reels (each with the reel it came from)."""
    if script.get("kind") != "launch":
        return [(script["id"], sh) for sh in script["shots"]]
    out = []
    for part in script["parts"]:
        other = scripts[part["reel"]]
        idx = part.get("shots") or list(range(len(other["shots"])))
        for i in idx:
            sh = dict(other["shots"][i])
            if part.get("caption") is not None and i == idx[0]:
                sh["caption"] = part["caption"]
            out.append((part["reel"], sh))
    return out


def make(script, scripts, takes_cat, takes_dir, out_dir, formats=None, placeholders=False, final=False, fast=False, log=functools.partial(print, flush=True)):
    """Cut one script into its videos. Returns {"files": [...], "missing": [take ids], "draft": bool}."""
    sid = script["id"]
    draft = is_draft(script, scripts)
    if final and draft:
        raise ScriptError("%s still has TODO(owner) words: the owner writes them first (or leave out --final)" % sid)
    enc = Enc(fast)
    formats = [f for f in (formats or script.get("formats") or list(B.FORMATS)) if f in (script.get("formats") or B.FORMATS)]
    dest = os.path.join(out_dir, sid)
    os.makedirs(dest, exist_ok=True)
    result = {"files": [], "missing": [], "draft": draft}
    plan = []   # (where it came from, the shot, its take, the take's meta, its index in shots_of)
    for k, (origin, sh) in enumerate(shots_of(script, scripts)):
        mp4, meta = find_take(takes_dir, sh["take"])
        if mp4 is None:
            if sh["take"] not in result["missing"]:
                result["missing"].append(sh["take"])
            if placeholders:
                plan.append((origin, sh, None, None, k))
            continue
        plan.append((origin, sh, mp4, meta, k))
    if not plan:
        log("%s: no takes recorded yet (%s): nothing to cut" % (sid, ", ".join(result["missing"])))
        return result
    if result["missing"]:
        log("%s: %s %s" % (sid, "placeholders for" if placeholders else "left out (not recorded yet):", ", ".join(result["missing"])))
    work = tempfile.mkdtemp(prefix="reel-%s-" % sid)
    try:
        for fmt in formats:
            parts = []
            p = os.path.join(work, "%s-sting.mp4" % fmt)
            parts.append((p, frames_clip(cards.sting_frames(fmt), p, enc)))
            p = os.path.join(work, "%s-title.mp4" % fmt)
            parts.append((p, still(cards.title_card(fmt, shown(script["title"]), shown(script.get("kicker", "")), shown(script.get("sub", ""))),
                                   float(script.get("title_seconds", B.TITLE_SECONDS)), p, enc, zoom=0.025)))
            first_shot = len(parts)
            for i, (_origin, sh, mp4, meta, _k) in enumerate(plan):
                p = os.path.join(work, "%s-shot%02d.mp4" % (fmt, i))
                if mp4 is None:
                    t = takes_cat.get(sh["take"], {})
                    parts.append((p, still(cards.placeholder(fmt, sh["take"], t.get("what", "")), 2.0, p, enc)))
                    continue
                mp4, meta = find_take(takes_dir, sh["take"], fmt)
                t_in, t_out = at(sh["in"], meta), at(sh["out"], meta)
                if t_out - t_in < 0.3:
                    raise ScriptError("%s: shot %d of take %s is %.2fs long (in %.2f, out %.2f)" % (sid, i, sh["take"], t_out - t_in, t_in, t_out))
                parts.append((p, shot_clip(mp4, t_in, t_out, fmt, sh, p, work, enc)))
            p = os.path.join(work, "%s-end.mp4" % fmt)
            parts.append((p, still(cards.end_card(fmt, shown(script.get("end_line", ""))), float(script.get("end_seconds", B.END_SECONDS)), p, enc)))
            badge = None
            if draft:
                badge = os.path.join(work, "%s-badge.png" % fmt)
                cards.draft_badge(fmt).save(badge)
            out = os.path.join(dest, "%s-%s.mp4" % (sid, fmt))
            total = join(parts, out, enc, badge)
            result["files"].append(out)
            # where each part starts in the joined video (each cross-fade overlaps the one before)
            starts, t = [], 0.0
            for _, d in parts:
                starts.append(t)
                t += d - B.XFADE
            srt = os.path.join(dest, "%s-%s.srt" % (sid, fmt))
            with open(srt, "w", encoding="utf-8") as f:
                n = 0
                for i, (_o, sh, _m, _meta, _k) in enumerate(plan):
                    if sh.get("caption"):
                        n += 1
                        a, b = starts[first_shot + i], starts[first_shot + i] + parts[first_shot + i][1] - B.XFADE
                        f.write("%d\n%s --> %s\n%s\n\n" % (n, srt_time(a), srt_time(b), shown(sh["caption"])))
            result["files"].append(srt)
            sh_path = os.path.join(dest, "%s-%s-sheet.jpg" % (sid, fmt))
            sheet(out, sh_path)
            result["files"].append(sh_path)
            g = script.get("gif") or {}
            if fmt == g.get("format", "wide") or (len(formats) == 1 and g):
                # the GIF's shots by their number in the script; the ones cut here (a missing take isn't)
                want = g.get("shots") or [plan[i][4] for i in range(len(plan)) if plan[i][2] is not None][:2]
                idx = [i for i in range(len(plan)) if plan[i][4] in want and plan[i][2] is not None]
                if idx:
                    a = starts[first_shot + idx[0]] + B.XFADE / 2
                    b = starts[first_shot + idx[-1]] + parts[first_shot + idx[-1]][1] - B.XFADE
                    secs = min(float(g.get("max_seconds", 8)), max(1.0, b - a))
                    gp = os.path.join(dest, "%s.gif" % sid)
                    gif(out, a, secs, gp, int(g.get("width", 640 if fmt == "wide" else 360)), int(g.get("fps", 12)))
                    result["files"].append(gp)
            log("%s: %s %.1fs%s" % (sid, os.path.relpath(out, out_dir), total, " (DRAFT)" if draft else ""))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    return result


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("reels", nargs="*", help="script ids (default: every script; the launch video last)")
    ap.add_argument("--takes", default=TAKES)
    ap.add_argument("--out", default=OUT)
    ap.add_argument("--scripts", default=SCRIPTS)
    ap.add_argument("--formats", default="", help="reel,wide (default: each script's own)")
    ap.add_argument("--placeholders", action="store_true", help="a card for each take not recorded yet (a preview)")
    ap.add_argument("--final", action="store_true", help="refuse scripts with TODO(owner) words; no DRAFT tag")
    ap.add_argument("--fast", action="store_true", help="quick encode (previews and tests)")
    ap.add_argument("--list", action="store_true", help="the scripts and which takes each still needs")
    ap.add_argument("--check", action="store_true", help="check the scripts only")
    a = ap.parse_args(argv)
    for f in (sys.stdout, sys.stderr):
        try:
            f.reconfigure(errors="replace")
        except (AttributeError, ValueError):
            pass
    takes_cat = catalog(a.scripts)
    scripts = all_scripts(a.scripts)
    problems = []
    for s in scripts.values():
        problems += validate(s, takes_cat, scripts)
    if problems:
        print("Script problems:\n  " + "\n  ".join(problems))
        return 2
    if a.check:
        print("%d scripts, %d takes: OK" % (len(scripts), len(takes_cat)))
        return 0
    ids = a.reels or sorted(scripts, key=lambda k: (scripts[k]["kind"] == "launch", k))
    unknown = [i for i in ids if i not in scripts]
    if unknown:
        print("No script called: " + ", ".join(unknown) + " (have: " + ", ".join(sorted(scripts)) + ")")
        return 2
    if a.list:
        for i in ids:
            need = sorted({sh["take"] for _, sh in shots_of(scripts[i], scripts)})
            have = [t for t in need if find_take(a.takes, t)[0]]
            print("%-24s %-6s %d/%d takes%s%s" % (i, scripts[i]["kind"], len(have), len(need), "  DRAFT" if is_draft(scripts[i], scripts) else "",
                                                 ("  needs: " + ", ".join(t for t in need if t not in have)) if len(have) < len(need) else ""))
        return 0
    if not tool("ffmpeg"):
        print("ffmpeg is needed (winget install Gyan.FFmpeg, or ffmpeg.org) and wasn't found.")
        return 1
    fmts = [f for f in a.formats.split(",") if f] or None
    made, failed = 0, 0
    for i in ids:
        try:
            r = make(scripts[i], scripts, takes_cat, a.takes, a.out, fmts, a.placeholders, a.final, a.fast)
            made += bool(r["files"])
        except (ScriptError, RuntimeError) as e:
            failed += 1
            print("FAIL %s: %s" % (i, e), flush=True)
    print("REELS: %d made, %d failed, out in %s" % (made, failed, a.out))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
