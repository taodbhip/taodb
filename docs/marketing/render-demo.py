#!/usr/bin/env python3
"""
render-demo.py — Render the taodb 30s demo as PNG frames, then encode
to mp4 + gif using ffmpeg (no vhs / asciinema / browser automation needed).

Output: docs/marketing/demo.mp4  +  docs/marketing/demo.gif

Storyboard (matches demo-script.md):
  0:00  title card                       "taodb — memory for LLM agents"
  0:03  taodb init                       "✓ 完成. 重启 agent 即可使用 taodb. ..."
  0:07  Session 1 demo_agent.py memorize ...
  0:13  two weeks pass                    (caption, fades)
  0:15  Session 2 demo_agent.py ask ...   (recall_paths block the punchline)
  0:23  side-by-side                     (vector DB 0/5 vs taodb 5/5)
  0:27  github.com/taodbhip/taodb         (final on-screen ≥ 2s)
  0:30  fade out

Each storyboard beat = one or more terminal "frames" rendered to PNG.
A frame = full terminal screenshot at moment T. We compute its on-screen
duration from cumulative ms and emit sequence 0..N PNGs.
"""
from __future__ import annotations
import os
import pathlib
import shutil
import subprocess
import sys

from PIL import Image, ImageDraw, ImageFont

# --- paths ----------------------------------------------------------------
HERE = pathlib.Path(__file__).resolve().parent
OUT_DIR = HERE
FRAMES_DIR = HERE / "render" / "frames"
FRAMES_DIR.mkdir(parents=True, exist_ok=True)

# --- canvas ---------------------------------------------------------------
WIDTH, HEIGHT = 1280, 720
PADDING_X, PADDING_Y = 32, 28
FONT_SIZE = 18
LINE_GAP = 8

# Tokyo Night palette (dark)
BG = (26, 27, 38)            # #1a1b26
FG = (192, 202, 245)         # #c0caf5
FG_DIM = (86, 95, 137)       # comment-y dim grey
ACCENT_CYAN = (125, 207, 255)   # the ── divider + accent boxes
ACCENT_GREEN = (158, 206, 106)
ACCENT_PURPLE = (187, 154, 247)
ACCENT_YELLOW = (224, 175, 104)
ACCENT_RED = (247, 118, 142)

# --- font -----------------------------------------------------------------
# We need a single font that covers both ASCII terminal text AND the
# Chinese characters that `taodb init` and `recall_paths` ("天", "地")
# emit. macOS has zero "true mono" CJK fonts, so we use Hiragino Sans GB:
# it ships Latin at half-width (Latin 0.5em, CJK 1em) so mixed lines
# render with even column spacing that *looks* like a terminal.
FONT_PATHS = [
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/System/Library/Fonts/STHeiti Medium.ttc",
    "/System/Library/Fonts/Menlo.ttc",         # ascii-only fallback
    "/System/Library/Fonts/SFNSMono.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
]


def load_font(size: int = FONT_SIZE) -> ImageFont.ImageFont:
    for p in FONT_PATHS:
        if pathlib.Path(p).exists():
            try:
                return ImageFont.truetype(p, size=size, index=0)
            except Exception:
                continue
    return ImageFont.load_default()


FONT = load_font()
FONT_BOLD = load_font(FONT_SIZE)


# --- primitive: render a styled text line into an image --------------------
def _measure_line(text: str, font: ImageFont.ImageFont) -> int:
    """Width of a string in pixels using the given font."""
    bbox = font.getbbox(text)
    return bbox[2] - bbox[0]


def _strip_ansi(s: str) -> str:
    """Strip ANSI escapes — the demo output uses some, but PIL can't render them.
    We map them to accent colors via a separate pass."""
    import re
    return re.sub(r"\x1b\[[0-9;]*m", "", s)


# tokenized line: list of (text, color) tuples OR plain string
def _emit_line(draw: ImageDraw.ImageDraw, y: int, line, x0: int = PADDING_X) -> int:
    """Draw a styled line at y. Returns y for next line."""
    if isinstance(line, str):
        line = [(line, FG)]
    cursor = x0
    for chunk in line:
        text, color = chunk
        draw.text((cursor, y), text, fill=color, font=FONT)
        cursor += _measure_line(text, FONT)
    return y + FONT.size + LINE_GAP


# --- storyboard ----------------------------------------------------------
# Each beat = (duration_ms_after_beat_starts, callable(bottom_y) -> next_y)
# bottom_y is the y where the next beat starts (top of new content).
# Beats have a leading blank state and add content on top.

def render_title_card(draw: ImageDraw.ImageDraw):
    """First 0:00–0:03."""
    y = 240
    title = "# taodb — memory for LLM agents"
    # measure center
    tw = _measure_line(title, FONT)
    x = (WIDTH - tw) // 2
    draw.text((x, y), title, fill=ACCENT_CYAN, font=FONT)


def render_init(draw: ImageDraw.ImageDraw):
    """0:03–0:07"""
    y = 64
    lines = [
        [("user@mac  ~/demo/auth-service  ", FG), ("$ ", FG_DIM), ("taodb init --user demo --project auth-service", FG)],
        "",
        [("[ok] ", ACCENT_GREEN), ("创建数据目录: ", FG), ("./taodb-memory/", FG)],
        [("[ok] ", ACCENT_GREEN), ("创建 ", FG), (".mcp.json ", FG), ("(user=demo, project=auth-service)", FG_DIM)],
        [("[ok] ", ACCENT_GREEN), ("创建 ", FG), (".taodb/instructions.md", FG)],
        [("[ok] ", ACCENT_GREEN), ("创建 ", FG), (".gitignore", FG)],
        "",
        [("[ok] ", ACCENT_GREEN), ("完成。重启 agent 即可使用 taodb。", FG)],
        [("    首次会话 agent 会提示导入项目内容。", FG_DIM)],
        [("    如需自定义行为，编辑 ", FG_DIM), (".taodb/instructions.md", FG), (" (可提交 git)。", FG_DIM)],
    ]
    for l in lines:
        if l == "":
            y += FONT.size + LINE_GAP
        else:
            y = _emit_line(draw, y, l)


def render_session1_memorize(draw: ImageDraw.ImageDraw):
    """0:07–0:13. The Session 1 stanza + memorize output."""
    y = 64
    lines = [
        [("# ", FG_DIM), ("Session 1 ", ACCENT_YELLOW), ("— agent finishes a debugging session", FG)],
        "",
        [("agent$ ", FG_DIM), ("python3 demo_agent.py memorize \"Fixed token-rotation race; switched mutex to atomic swap. Mutex deadlocked under contention.\" --containers feature:auth,bug:race-condition --energy 0.3", FG)],
        "",
        [("─" * 60, FG_DIM)],
        [("  [memo]  ", ACCENT_PURPLE), ("AGENT  →  ", FG), ("taodb_memorize", ACCENT_CYAN)],
        [("─" * 60, FG_DIM)],
        [("   text:        ", FG_DIM), ("Fixed token-rotation race; switched mutex to atomic swap. Mutex deadlocked under contention.", FG)],
        [("   containers:  ", FG_DIM), ("['feature:auth', 'bug:race-condition']", ACCENT_PURPLE)],
        [("   energy:      ", FG_DIM), ("0.3", ACCENT_YELLOW)],
        "",
        [("[ok] ", ACCENT_GREEN), ("stored memory_id=01KWMDFKG4RSPWY426TVXKNACD", FG)],
    ]
    for l in lines:
        if l == "":
            y += FONT.size + LINE_GAP
        else:
            y = _emit_line(draw, y, l)


def render_two_weeks(draw: ImageDraw.ImageDraw):
    """0:13–0:15. Just the caption, centered."""
    caption = "# ... two weeks pass ..."
    tw = _measure_line(caption, FONT)
    x = (WIDTH - tw) // 2
    y = (HEIGHT - FONT.size) // 2
    draw.text((x, y), caption, fill=FG_DIM, font=FONT)


def render_session2_ask(draw: ImageDraw.ImageDraw):
    """0:15–0:23. The Session 2 ask + recall output (THE PUNCHLINE)."""
    y = 64
    lines = [
        [("# ", FG_DIM), ("Session 2 ", ACCENT_YELLOW), ("— new agent, same project", FG)],
        "",
        [("agent$ ", FG_DIM), ("python3 demo_agent.py ask \"what did we decide about auth last week?\" --containers feature:auth --days 14", FG)],
        "",
        [("─" * 60, FG_DIM)],
        [("  [find]  ", ACCENT_CYAN), ("AGENT  →  ", FG), ("taodb_recall", ACCENT_CYAN), ('  "what did we decide about auth last week?"', FG)],
        [("─" * 60, FG_DIM)],
        [("   containers:  ", FG_DIM), ("['feature:auth']", ACCENT_PURPLE)],
        [("   last:        ", FG_DIM), ("14 days", ACCENT_YELLOW)],
        "",
        [("taodb returned 1 memories:", FG)],
        "",
        [("  1. ", FG), ("Fixed token-rotation race; switched mutex to atomic swap. Mutex deadlocked under contention.", FG)],
        [("     ", FG_DIM), ("energy=0.35 ", ACCENT_YELLOW), ("(floor=0.30) · containers=['feature:auth', 'bug:race-condition']", FG_DIM)],
        "",
        [("─── recall paths ───", FG_DIM)],
        [("  anchor: 1783096659460241000 (containers: [\"feature:auth\"])", FG_DIM)],
        [("  ", FG_DIM), ("天: ", ACCENT_CYAN), ("time_range [1781887059..., 1784306259...] → ", FG_DIM), ("1 hits", ACCENT_GREEN)],
        [("  ", FG_DIM), ("地: ", ACCENT_PURPLE), ("container_overlap → ", FG_DIM), ("1 hits", ACCENT_GREEN)],
        [("  ", FG_DIM), ("result: ", FG), ("1 memories", ACCENT_GREEN)],
    ]
    for l in lines:
        if l == "":
            y += FONT.size + LINE_GAP
        else:
            y = _emit_line(draw, y, l)


def render_sidebyside(draw: ImageDraw.ImageDraw):
    """0:23–0:27. Side-by-side vector DB 0/5 vs taodb 5/5."""
    # Background split: left half subtly red-tinted, right half green-tinted
    # Draw two header bars
    y = 60
    half_w = (WIDTH - PADDING_X * 2 - 20) // 2
    left_x = PADDING_X
    right_x = PADDING_X + half_w + 20

    # titles
    draw.text((left_x, y), "VECTOR DB (pgvector)", fill=ACCENT_RED, font=FONT)
    draw.text((right_x, y), "taodb (MCP stdio)", fill=ACCENT_GREEN, font=FONT)
    y += FONT.size + 12

    # subtitles
    draw.text((left_x, y), 'query="what did we decide about auth"', fill=FG_DIM, font=FONT)
    draw.text((right_x, y), 'query=containers=["feature:auth"], last=14d', fill=FG_DIM, font=FONT)
    y += FONT.size + 16

    # body
    left_lines = [
        "1. Mutex patterns in Rust",
        "2. Fix race condition (SO)",
        "3. OAuth2 RFC (text excerpt)",
        "4. Token validation tutorial",
        "5. Mutex deadlock StackOverflow",
    ]
    for ln in left_lines:
        draw.text((left_x, y), ln, fill=FG, font=FONT)
        y_l = y
        y += FONT.size + LINE_GAP

    # reset y for right column
    right_y = y - (len(left_lines)) * (FONT.size + LINE_GAP) - LINE_GAP

    right_lines = [
        [("1. [match] ", ACCENT_GREEN), ("Fixed token-rotation race;", FG)],
        [("    switched mutex → atomic", FG)],
        [("    swap. Mutex deadlocked.", FG)],
        "",
        [("   energy=0.35 ", ACCENT_YELLOW), ("(floor=0.30)", FG_DIM)],
        [("   recall_paths:", FG_DIM)],
        [("     ", FG_DIM), ("天: ", ACCENT_CYAN), ("1 hits  ", FG_DIM), ("地: ", ACCENT_PURPLE), ("1 hits", FG_DIM)],
    ]
    cursor_y = right_y
    for l in right_lines:
        if l == "":
            cursor_y += FONT.size + LINE_GAP
        else:
            cursor_y = _emit_line(draw, cursor_y, l, x0=right_x)
        # apply to left too
    # score footer
    score_y = max(y, cursor_y) + 12
    draw.text((left_x, score_y), "recall@5 = 0/5", fill=ACCENT_RED, font=FONT)
    draw.text((right_x, score_y), "recall@5 = 5/5", fill=ACCENT_GREEN, font=FONT)


def render_github_url(draw: ImageDraw.ImageDraw):
    """0:27–0:30. Final URL centered on screen."""
    url = "# github.com/taodbhip/taodb"
    tw = _measure_line(url, FONT)
    x = (WIDTH - tw) // 2
    y = (HEIGHT - FONT.size) // 2
    draw.text((x, y), url, fill=ACCENT_CYAN, font=FONT)


# --- storyboard runner ----------------------------------------------------
# Each beat: (name, duration_ms, render_fn)
BEATS = [
    ("00_title_card",         3000, render_title_card),
    ("01_taodb_init",         4000, render_init),
    ("02_session1_memorize",  6000, render_session1_memorize),
    ("03_two_weeks_pass",     2000, render_two_weeks),
    ("04_session2_ask",       8000, render_session2_ask),
    ("05_side_by_side",       4000, render_sidebyside),
    ("06_github_url",         3000, render_github_url),
]


def render_blank() -> Image.Image:
    return Image.new("RGB", (WIDTH, HEIGHT), BG)


def main():
    # 1) Render each beat's still frame
    stills = []
    for name, dur_ms, fn in BEATS:
        img = render_blank()
        draw = ImageDraw.Draw(img)
        fn(draw)
        out = FRAMES_DIR / f"{name}.png"
        img.save(out, optimize=True)
        stills.append((name, dur_ms, out))
        print(f"  ✓ {name}.png  ({dur_ms}ms)")

    # 2) Expand to a frame sequence. For each beat, emit one PNG every
    #    ~250ms (4 fps) so the GIF has smooth motion when chars appear;
    #    but terminal "type-in" is a *fade-in*, not a typewriter, so
    #    we'll instead emit a smaller number of frames per beat
    #    proportional to duration, with the still at the END of each beat.
    seq_dir = FRAMES_DIR / "seq"
    if seq_dir.exists():
        shutil.rmtree(seq_dir)
    seq_dir.mkdir()

    fps_target = 8  # frames per second; small file + still readable
    total_ms = 0
    seq_paths = []
    frame_idx = 0  # ffmpeg `-start_number 1` (default) — keep 1-based below
    for name, dur_ms, still in stills:
        n_frames = max(2, int(dur_ms / 1000 * fps_target))
        for i in range(n_frames):
            # linearly fade-in the still from BG over first 30% of beat
            t = i / max(1, n_frames - 1)  # 0..1
            base = Image.open(still)
            # alpha 0.55 (early) → 1.0 (late); bg already bg
            alpha = 0.55 + 0.45 * t
            blend = Image.blend(render_blank(), base, alpha)
            frame_idx += 1
            out = seq_dir / f"f{frame_idx:06d}.png"
            blend.save(out, optimize=False)
            seq_paths.append(out)
        total_ms += dur_ms

    print(f"  ✓ {len(seq_paths)} sequence frames, total ~{total_ms/1000:.1f}s")

    # 3) Encode to mp4 + gif via ffmpeg
    mp4 = OUT_DIR / "demo.mp4"
    cmd_mp4 = [
        "ffmpeg", "-y", "-hide_banner", "-loglevel", "error",
        "-framerate", str(fps_target),
        "-i", str(seq_dir / "f%06d.png"),
        "-c:v", "libx264", "-pix_fmt", "yuv420p",
        "-vf", f"scale={WIDTH}:{HEIGHT}",
        "-preset", "slow", "-crf", "22",
        str(mp4),
    ]
    print("  ▸ ffmpeg → demo.mp4")
    subprocess.run(cmd_mp4, check=True)

    gif = OUT_DIR / "demo.gif"
    # palette mode for small gif
    palette = OUT_DIR / "demo-palette.png"
    cmd_palette = [
        "ffmpeg", "-y", "-hide_banner", "-loglevel", "error",
        "-i", str(mp4),
        "-vf", f"fps={fps_target},scale={WIDTH}:-1:flags=lanczos,palettegen=max_colors=128",
        str(palette),
    ]
    print("  ▸ ffmpeg palette")
    subprocess.run(cmd_palette, check=True)

    cmd_gif = [
        "ffmpeg", "-y", "-hide_banner", "-loglevel", "error",
        "-i", str(mp4),
        "-i", str(palette),
        "-filter_complex", f"fps={fps_target},scale={WIDTH}:-1:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=5",
        "-loop", "0",
        str(gif),
    ]
    print("  ▸ ffmpeg → demo.gif")
    subprocess.run(cmd_gif, check=True)

    # Cleanup unless KEEP_FRAMES=1
    if not os.environ.get("KEEP_FRAMES"):
        shutil.rmtree(FRAMES_DIR)

    print(f"\nDone.")
    print(f"  mp4: {mp4}  ({mp4.stat().st_size/1024:.1f} KB)")
    print(f"  gif: {gif}  ({gif.stat().st_size/1024:.1f} KB)")


if __name__ == "__main__":
    main()
