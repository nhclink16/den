"""Contact sheet for the visual tour: every screenshot with its stop, light and dark side by side, on one page.
  python3 tour-sheet.py <out-dir> [web-dir]
Reads <out-dir>/shots (TourUITests' PNGs from `xcloud.sh tour`, named <test>-<stop>-<light|dark>.png) and, if given,
web-dir (web-<stop>-<light|dark>.png, for stops the native tour can't reach). Writes <out-dir>/sheet.html and
<out-dir>/shots.txt (one line per screenshot: file, stop, theme, native or web). Paths in both are relative to <out-dir>.
"""
import html, os, re, sys

out = sys.argv[1]
web = sys.argv[2] if len(sys.argv) > 2 else None
NAME = re.compile(r'^(?P<test>test\w+|web)-(?P<stop>.+)-(?P<theme>light|dark)\.png$')


def collect(folder, kind):
    """Stops in the order they were shot: by test, then by when each file was written (xcresult-shots.py writes them
    in the order the test added them)."""
    rows = {}
    files = [f for f in os.listdir(folder) if NAME.match(f)]
    files.sort(key=lambda f: (NAME.match(f)['test'], os.stat(os.path.join(folder, f)).st_mtime_ns))
    for f in files:
        m = NAME.match(f)
        row = rows.setdefault((m['test'], m['stop']), {'kind': kind, 'section': m['test']})
        row[m['theme']] = os.path.relpath(os.path.join(folder, f), out)
    return rows


rows = collect(os.path.join(out, 'shots'), 'native')
if web:
    rows.update(collect(web, 'web'))

with open(os.path.join(out, 'shots.txt'), 'w') as f:
    for (_, stop), row in rows.items():
        for theme in ('light', 'dark'):
            if theme in row:
                f.write(f"{row[theme]}\t{stop}\t{theme}\t{row['kind']}\n")


def cell(path):
    if not path:
        return '<td class="none">not shot</td>'
    p = html.escape(path)
    return f'<td><a href="{p}"><img src="{p}" loading="lazy" alt=""></a></td>'


body, section = [], None
for (test, stop), row in rows.items():
    if row['section'] != section:
        section = row['section']
        title = 'Web, not native (Playwright WebKit, 402×874)' if row['kind'] == 'web' else re.sub(r'^test\d+', '', section)
        body.append(f'<tr><th colspan="3" class="section">{html.escape(title)}</th></tr>')
    miss = ' class="missing"' if stop.startswith('missing-') else ''
    body.append(f'<tr{miss}><th>{html.escape(stop)}</th>{cell(row.get("light"))}{cell(row.get("dark"))}</tr>')

shots = sum(('light' in r) + ('dark' in r) for r in rows.values())
page = f'''<!doctype html>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Den visual tour</title>
<style>
  :root {{ color-scheme: light dark; --bg: #fff; --ink: #1d1d1f; --line: #ddd; --faint: #777; --warn: #b3261e; }}
  @media (prefers-color-scheme: dark) {{ :root {{ --bg: #161617; --ink: #f2f2f2; --line: #333; --faint: #999; --warn: #f2b8b5; }} }}
  body {{ margin: 0; padding: 16px; background: var(--bg); color: var(--ink); font: 14px/1.4 system-ui, sans-serif; }}
  h1 {{ font-size: 20px; margin: 0 0 4px; }}
  p {{ color: var(--faint); margin: 0 0 16px; }}
  table {{ border-collapse: collapse; }}
  th, td {{ border-bottom: 1px solid var(--line); padding: 8px; vertical-align: top; text-align: left; }}
  th {{ font-weight: 600; width: 180px; word-break: break-word; }}
  th.section {{ font-size: 16px; padding-top: 24px; width: auto; }}
  img {{ width: 260px; border: 1px solid var(--line); border-radius: 8px; display: block; }}
  .none {{ color: var(--faint); width: 260px; }}
  .missing th {{ color: var(--warn); }}
</style>
<h1>Den visual tour</h1>
<p>{shots} screenshots. Light on the left, dark on the right. Made-up people and messages only. Click a picture for full size.</p>
<table>
<tr><th>Stop</th><th>Light</th><th>Dark</th></tr>
{chr(10).join(body)}
</table>
'''
open(os.path.join(out, 'sheet.html'), 'w').write(page)
print(os.path.join(out, 'sheet.html'))
