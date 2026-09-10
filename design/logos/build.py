#!/usr/bin/env python3
"""Build index.html: every SVG in astra/ and fable/, inlined, at favicon sizes on dark and light."""
import glob, html, os, re
root = os.path.dirname(os.path.abspath(__file__))
def notes(lane):
    p = os.path.join(root, lane, 'NOTES.md')
    if not os.path.exists(p): return {}
    out = {}
    for line in open(p):
        m = re.search(r'\*\*(.+?)\*\*\s*[—:-]*\s*(.*)', line)
        if not m: continue
        bold, blurb = m.group(1), m.group(2).strip()
        pre = re.match(r'\s*(\d+)\.', line)
        num = int(pre.group(1)) if pre else (int(re.match(r'\d+', bold).group()) if re.match(r'\d+', bold) else 0)
        title = re.sub(r'^\d+-[\w-]+\.svg\s*[—:-]+\s*', '', bold).rstrip('.')
        out[num] = (title, blurb)
    return out
cards = []
for lane in ('fable', 'astra'):
    files = sorted(glob.glob(os.path.join(root, lane, '*.svg')))
    n = notes(lane)
    for f in files:
        name = os.path.basename(f)
        num = int(re.match(r'(\d+)', name).group(1)) if re.match(r'\d+', name) else 0
        title, blurb = n.get(num, (name.replace('.svg', ''), ''))
        svg = re.sub(r'<style>.*?</style>', '', open(f).read(), flags=re.S)
        # Namespace ids so gradients/filters from different files don't collide when inlined.
        ns = f"{lane}{num}-"
        svg = re.sub(r'id="([^"]+)"', lambda m: f'id="{ns}{m.group(1)}"', svg)
        svg = re.sub(r'url\(#([^)]+)\)', lambda m: f'url(#{ns}{m.group(1)})', svg)
        svg = re.sub(r'href="#([^"]+)"', lambda m: f'href="#{ns}{m.group(1)}"', svg)
        svg = re.sub(r'aria-labelledby="[^"]*"', '', svg)
        svg_l = svg.replace('#1b1916', '#f3ede2').replace('#ece5d8', '#1b1916')  # light tile: swap room and ink
        sizes = ''.join(f'<span class="s" style="width:{s}px;height:{s}px">{svg}</span>' for s in (16, 24, 32, 48, 64))
        cards.append(f'''
<article class="card">
  <div class="big">{svg}</div>
  <div class="meta">
    <div class="eyebrow">{lane} · {name}</div>
    <h2>{html.escape(title)}</h2>
    <p>{html.escape(blurb)}</p>
    <div class="row dark">{sizes}</div>
    <div class="row light"><span class="s" style="width:16px;height:16px">{svg_l}</span><span class="s" style="width:32px;height:32px">{svg_l}</span><span class="s" style="width:48px;height:48px">{svg_l}</span></div>
    <div class="tab"><span class="fav">{svg}</span><span>Den</span><span class="x">×</span></div>
  </div>
</article>''')
page = f'''<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Den logo concepts</title>
<link href="https://fonts.googleapis.com/css2?family=Zilla+Slab:wght@600&family=Atkinson+Hyperlegible&family=IBM+Plex+Mono&display=swap" rel="stylesheet">
<style>
body{{margin:0;background:#1b1916;color:#ece5d8;font:15px/1.45 'Atkinson Hyperlegible',system-ui,sans-serif;padding:32px}}
h1{{font:600 34px 'Zilla Slab',serif;margin:0 0 4px}} .sub{{color:#a89f8f;margin:0 0 28px}}
.grid{{display:grid;grid-template-columns:repeat(auto-fill,minmax(360px,1fr));gap:18px}}
.card{{display:flex;gap:18px;background:#232019;border:1px solid #3a3429;border-radius:12px;padding:18px}}
.big{{width:144px;flex:none}} .big svg{{width:144px;height:144px;filter:drop-shadow(0 10px 24px rgba(0,0,0,.45))}}
.meta{{flex:1;min-width:0}} .eyebrow{{font:11px 'IBM Plex Mono',monospace;letter-spacing:.08em;text-transform:uppercase;color:#6f6759}}
h2{{font:600 22px 'Zilla Slab',serif;margin:2px 0 4px}} p{{margin:0 0 10px;color:#a89f8f;font-size:14px}}
.row{{display:flex;align-items:center;gap:14px;padding:10px 12px;border-radius:8px;margin-bottom:6px}}
.row.dark{{background:#1b1916;color:#ece5d8}} .row.light{{background:#f3ede2;color:#1b1916}} .big{{color:#ece5d8}} .tab{{color:#ece5d8}}
.s{{display:inline-block}} .s svg{{width:100%;height:100%;display:block}}
.tab{{display:inline-flex;align-items:center;gap:8px;background:#2c2821;border-radius:8px 8px 0 0;padding:6px 12px;font-size:13px;margin-top:4px}}
.fav{{width:16px;height:16px;display:inline-block}} .fav svg{{width:16px;height:16px;display:block}} .x{{color:#6f6759}}
</style></head><body>
<h1>Den logo concepts</h1><p class="sub">Each at 144, then 64 down to 16 on dark, 48 to 16 on light, and as a browser tab.</p>
<div class="grid">{''.join(cards)}</div></body></html>'''
open(os.path.join(root, 'index.html'), 'w').write(page)
print(f'{len(cards)} concepts')
