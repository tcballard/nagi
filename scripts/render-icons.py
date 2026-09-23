#!/usr/bin/env python3
"""Reproduce approved icon exports. Requires librsvg's rsvg-convert."""
from pathlib import Path
import subprocess
import base64
ROOT = Path(__file__).resolve().parents[1]
ICONS = ROOT / 'assets/icons'
MASTER = ICONS / 'hicolor/scalable/apps/nagi.svg'
SIZES = (16, 22, 24, 32, 48, 64, 128, 256, 512)
def small(n):
    grid = 16 if n <= 22 else 32 if n <= 32 else 64
    scale = n / grid
    cx, cy, r, x1, x2, y, refs = {
        16: (8, 6, 3, 3, 13, 11, []),
        32: (16, 12, 6, 5, 27, 19, [(12,20,24,.5)]),
        64: (32, 25, 10, 10, 54, 37, [(24,40,42,.55),(27,37,47,.35)]),
    }[grid]
    moon_y = round(cy*scale)
    # 24px: move up one pixel so the larger moon does not overlap the horizon.
    if n == 24:
        moon_y -= 1
    def line(a,b,y,colour,opacity=1):
        return f'<line x1="{a*scale:g}" y1="{round(y*scale)}" x2="{b*scale:g}" y2="{round(y*scale)}" stroke="{colour}" stroke-opacity="{opacity:g}" stroke-width="2" stroke-linecap="round"/>'
    return '\n'.join([f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {n} {n}" width="{n}" height="{n}">',f'<rect width="{n}" height="{n}" rx="{n*.225:g}" fill="#1a1b26"/>',f'<circle cx="{cx*scale:g}" cy="{moon_y}" r="{r*scale:g}" fill="#7aa2f7"/>',line(x1,x2,y,'#c0caf5'),*[line(a,b,y,'#7aa2f7',o) for a,b,y,o in refs],'</svg>'])+'\n'
for n in SIZES:
    source = MASTER
    if n < 128:
        source = ICONS / f'variants/nagi-{n}.svg'
        source.write_text(small(n))
    dest = ICONS / f'hicolor/{n}x{n}/apps/nagi.png'
    dest.parent.mkdir(parents=True,exist_ok=True)
    subprocess.run(['rsvg-convert','-w',str(n),'-h',str(n),str(source),'-o',str(dest)],check=True)
# Keep a 96-grid drawing available for consumers requesting this intermediate size.
(ICONS/'variants/nagi-96.svg').write_text(small(96))

(ICONS / "favicon.html").write_text('<link rel="icon" sizes="16x16" href="data:image/png;base64,' + base64.b64encode((ICONS / "hicolor/16x16/apps/nagi.png").read_bytes()).decode() + '">')
