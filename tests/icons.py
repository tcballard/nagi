#!/usr/bin/env python3
"""Pixel-level acceptance checks for approved icon exports (requires Pillow)."""
from pathlib import Path
import hashlib, json, xml.etree.ElementTree as ET
from PIL import Image
ROOT=Path(__file__).resolve().parents[1]
BASE=ROOT/'assets/icons'
EXPECTED='371c94d95297166de86e880c8aa9838236766d9c602d7e3ab2945171c6cc38b9'
assert hashlib.sha256((BASE/'hicolor/scalable/apps/nagi.svg').read_bytes()).hexdigest()==EXPECTED
rows={16:11,22:15,24:14,32:19,48:28,64:37}
report=[]
for n in (16,22,24,32,48,64,128,256,512):
    path=BASE/f'hicolor/{n}x{n}/apps/nagi.png'
    im=Image.open(path).convert('RGBA')
    assert im.size==(n,n) and im.getpixel((0,0))[3]==0
    if n in rows:
        y=rows[n];x=round(n*.32) # clear of the moon and reflection at every size
        assert im.getpixel((x,y-1))==(192,202,245,255),(n,x,y)
        assert im.getpixel((x,y))==(192,202,245,255),(n,x,y)
        assert im.getpixel((x,y+1))==(26,27,38,255),(n,x,y)
        root=ET.parse(BASE/f'variants/nagi-{n}.svg').getroot()
        moon=root[1]
        assert float(moon.attrib['cy'])+float(moon.attrib['r']) <= y-1
        lines=root.findall('{http://www.w3.org/2000/svg}line')
        assert len(lines)==(1 if n<=22 else 2 if n<=32 else 3)
        assert all(l.attrib['stroke-width']=='2' and float(l.attrib['y1']).is_integer() for l in lines)
        assert float(root[0].attrib['rx'])==n*.225
    report.append(dict(size=n,bytes=path.stat().st_size,sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
sym=ET.parse(BASE/'hicolor/symbolic/apps/nagi-symbolic.svg').getroot()
assert len(sym)==2 and sym[0].attrib['fill']=='currentColor' and sym[1].attrib['stroke']=='currentColor'
print(json.dumps({'master_sha256':EXPECTED,'exports':report,'small_horizon_rows':2},indent=2))
