<script lang="ts">
  // Browser-only settings section. Installers live on the GitHub release, so the
  // page asks GitHub for the latest one and picks the file for this computer.
  const RELEASES = 'https://github.com/nhclink16/den/releases/latest'
  type Asset = { name: string; browser_download_url: string; size: number }
  type Os = 'windows' | 'mac' | 'linux'
  const osNames: Record<Os, string> = { windows: 'Windows', mac: 'Mac', linux: 'Linux' }
  // First match is the one we recommend for that OS.
  const kinds: Record<Os, [RegExp, string][]> = {
    windows: [[/\.exe$/, 'Installer (.exe)'], [/\.msi$/, 'MSI package']],
    mac: [[/\.dmg$/, 'Disk image (.dmg)'], [/mac.*\.zip$/, 'Zip']],
    linux: [[/\.AppImage$/, 'AppImage'], [/\.deb$/, 'Debian package (.deb)']],
  }

  const platform = ((navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform || navigator.userAgent).toLowerCase()
  const mobile = /iphone|ipad|android/.test(navigator.userAgent.toLowerCase())
  const mine: Os | null = mobile ? null : platform.includes('win') ? 'windows' : platform.includes('mac') ? 'mac' : platform.includes('linux') ? 'linux' : null

  let tag = $state('')
  let files = $state<{ os: Os; label: string; asset: Asset }[]>([])
  let failed = $state(false)
  $effect(() => {
    fetch('https://api.github.com/repos/nhclink16/den/releases/latest')
      .then(r => { if (!r.ok) throw Error(String(r.status)); return r.json() })
      .then((rel: { tag_name: string; assets: Asset[] }) => {
        tag = rel.tag_name
        files = (Object.keys(kinds) as Os[]).flatMap(os => kinds[os].flatMap(([re, label]) => {
          const asset = rel.assets.find(a => re.test(a.name))
          return asset ? [{ os, label, asset }] : []
        }))
      })
      .catch(() => { failed = true })
  })
  const best = $derived(mine ? files.find(f => f.os === mine) : undefined)
  const mb = (n: number) => `${Math.round(n / 1e6)} MB`
</script>

<h2 class="display">Desktop app</h2>
<p class="muted">The desktop app adds push to talk that works while Den is in the background, screen share with system sound on Windows, and showing what you're playing. It signs in to the same server.</p>

{#if failed}
  <p><a class="btn lit" href={RELEASES} target="_blank" rel="noreferrer">Get it on GitHub</a></p>
{:else if !tag}
  <p class="faint">Finding the latest version…</p>
{:else}
  {#if best}
    <p><a class="btn lit" href={best.asset.browser_download_url}>Download for {osNames[best.os]}</a> <span class="faint small">{tag} · {mb(best.asset.size)}</span></p>
  {:else if mobile}
    <p class="faint">The desktop app runs on Windows, Mac and Linux. Open this page on a computer to install it.</p>
  {/if}
  <h3 class="eyebrow">All downloads</h3>
  <ul class="files">
    {#each files as f (f.asset.name)}
      <li><a href={f.asset.browser_download_url}>{osNames[f.os]}: {f.label}</a> <span class="faint small">{mb(f.asset.size)}</span></li>
    {/each}
  </ul>
  <p class="faint small"><a href={RELEASES} target="_blank" rel="noreferrer">Release notes</a></p>
{/if}

<style>
  .files { list-style: none; padding: 0; margin: 0 0 12px; display: grid; gap: 6px; }
  .small { font-size: 13px; }
</style>
