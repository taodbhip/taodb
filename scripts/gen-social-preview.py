#!/usr/bin/env python3
"""taodb Social Preview v4 — split layout: typography left, radial gradient right.

Design language: Stripe / Vercel / Cursor style.
- Deep black left, radial gradient (indigo→purple→violet) right
- Single brand word "taodb" as the hero
- Tracked-out caps subtitle, single monospace footer
- 4 elements total (caps / brand / sub / footer)
"""
import os
import numpy as np
from PIL import Image, ImageDraw, ImageFont

W, H = 1280, 640

# Palette
BG = (8, 11, 18)
FG = (240, 246, 252)
MUTED = (139, 148, 158)
DIM = (75, 85, 99)

# Gradient stops (radial, center → edge)
C_INNER = (165, 120, 255)   # violet-300 highlight at center
C_MID = (109, 50, 195)      # purple-700 mid
C_EDGE = (24, 18, 60)       # near-black indigo at edge


def load_font(candidates, size):
    for path in candidates:
        try:
            return ImageFont.truetype(path, size)
        except OSError:
            continue
    return ImageFont.load_default()


font_brand = load_font([
    "/System/Library/Fonts/Supplemental/Arial Black.ttf",
    "/System/Library/Fonts/HelveticaNeue.ttc",
], 220)
font_caps = load_font([
    "/System/Library/Fonts/HelveticaNeue.ttc",
    "/System/Library/Fonts/Helvetica.ttc",
], 18)
font_sub = load_font([
    "/System/Library/Fonts/HelveticaNeue.ttc",
    "/System/Library/Fonts/Helvetica.ttc",
], 30)
font_small = load_font([
    "/System/Library/Fonts/Menlo.ttc",
    "/System/Library/Fonts/Monaco.ttf",
], 14)


# === Build gradient with numpy (vectorized, fast) ===
cx_g, cy_g = int(W * 0.78), int(H * 0.5)  # gradient center, shifted right
max_r = 760.0                              # gradient radius

yy, xx = np.mgrid[0:H, 0:W].astype(np.float32)
dx = xx - cx_g
dy = yy - cy_g
dist = np.sqrt(dx * dx + dy * dy)
t = np.clip(dist / max_r, 0.0, 1.0)  # 0 at center, 1 at edge

# Two-segment radial gradient: inner → mid (t=0..0.45), mid → edge (t=0.45..1.0)
seg = np.where(t < 0.45, t / 0.45, (t - 0.45) / 0.55)
# smoothstep
seg = seg * seg * (3 - 2 * seg)

r = (C_INNER[0] + (C_MID[0] - C_INNER[0]) * np.minimum(seg * 2, 1)
     + (C_EDGE[0] - C_MID[0]) * np.maximum(seg * 2 - 1, 0))
g = (C_INNER[1] + (C_MID[1] - C_INNER[1]) * np.minimum(seg * 2, 1)
     + (C_EDGE[1] - C_MID[1]) * np.maximum(seg * 2 - 1, 0))
b = (C_INNER[2] + (C_MID[2] - C_INNER[2]) * np.minimum(seg * 2, 1)
     + (C_EDGE[2] - C_MID[2]) * np.maximum(seg * 2 - 1, 0))

# Build full RGB array
arr = np.stack([r, g, b], axis=-1).astype(np.uint8)

# Force left half to deep black (split layout)
arr[:, :W // 2] = np.array(BG, dtype=np.uint8)

img = Image.fromarray(arr)
draw = ImageDraw.Draw(img)


# === 左侧：typography 层级 ===
PAD = 80

# 顶部 tracked caps
caps = "MEMORY  LAYER  FOR  AI  AGENTS"
draw.text((PAD, 130), caps, fill=MUTED, font=font_caps)

# 巨字 "taodb"
draw.text((PAD - 6, 195), "taodb", fill=FG, font=font_brand)

# 副标
draw.text((PAD, 510), "Memory that actually remembers.", fill=MUTED, font=font_sub)

# footer
draw.text((PAD, 580), "github.com/taodbhip/taodb", fill=DIM, font=font_small)


# === 右侧：在渐变中心点放一个极小 logo 字符 "🧠" 修饰（用 Apple Emoji 字体） ===
# 加载 emoji 字体
emoji_font = None
for fp in ["/System/Library/Fonts/Apple Color Emoji.ttc"]:
    try:
        emoji_font = ImageFont.truetype(fp, 110)
        break
    except OSError:
        pass

if emoji_font:
    # 在渐变中心偏上放一个 🧠
    emoji = "🧠"
    ebbox = draw.textbbox((0, 0), emoji, font=emoji_font)
    ew = ebbox[2] - ebbox[0]
    eh = ebbox[3] - ebbox[1]
    draw.text(
        (int(cx_g - ew / 2), int(cy_g - eh / 2 - 30)),
        emoji, font=emoji_font,
    )


out = "/Users/xmfh-1/taodb/.github/social-preview.png"
os.makedirs(os.path.dirname(out), exist_ok=True)
img.save(out, "PNG", optimize=True)
print(f"saved: {out} ({os.path.getsize(out):,} bytes)")
