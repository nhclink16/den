<script lang="ts">
  import SettingRow from './SettingRow.svelte'
  // Browser-only settings section. Installers live on the GitHub release, so the
  // page asks GitHub for the latest one and picks the file for this computer.
  const RELEASES = 'https://github.com/nhclink16/den/releases/latest'
  type Asset = { name: string; browser_download_url: string; size: number }
  type Os = 'windows' | 'mac' | 'linux'
  const osNames: Record<Os, string> = { windows: 'Windows', mac: 'Mac', linux: 'Linux' }
  // Only electron-builder's Den-<version>-<os>-<arch>.<ext> files: the release also
  // carries the CLI, host and server binaries. First match is the recommendation.
  const kinds: Record<Os, [RegExp, string][]> = {
    // No MSI: the updater only ships NSIS, so an MSI install would end up with two registrations.
    windows: [[/^Den-[^-]+-win-.*\.exe$/, 'Installer (.exe)']],
    mac: [[/^Den-[^-]+-mac-.*\.dmg$/, 'Disk image (.dmg)'], [/^Den-[^-]+-mac-.*\.zip$/, 'Zip']],
    linux: [[/^Den-[^-]+-linux-.*\.AppImage$/, 'AppImage'], [/^Den-[^-]+-linux-.*\.deb$/, 'Debian package (.deb)']],
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
        if (!files.length) failed = true
      })
      .catch(() => { failed = true })
  })
  const best = $derived(mine ? files.find(f => f.os === mine) : undefined)
  const mb = (n: number) => `${Math.round(n / 1e6)} MB`
</script>

<h2 class="display">Desktop app</h2>

{#if failed}
  <div class="slist gap-top" data-find="download">
    <SettingRow label="Den for your computer" hint="Couldn't reach GitHub to find the right file.">
      {#snippet control()}<a class="btn lit" href={RELEASES} target="_blank" rel="noreferrer">Get it on GitHub</a>{/snippet}
    </SettingRow>
  </div>
{:else if !tag}
  <p class="list-foot">Finding the latest version…</p>
{:else}
  {#if best}
    <div class="slist gap-top" data-find="download">
      <SettingRow label={`Den for ${osNames[best.os]}`} hint={`${tag} · ${mb(best.asset.size)}`}>
        {#snippet control()}<a class="btn lit" href={best.asset.browser_download_url}>Download</a>{/snippet}
      </SettingRow>
    </div>
  {:else if mobile}
    <p class="list-foot">The desktop app runs on Windows, Mac and Linux. Open this page on a computer to install it.</p>
  {/if}
  <h3 class="list-head">All downloads</h3>
  <div class="slist" data-find={best ? undefined : 'download'}>
    {#each files as f (f.asset.name)}
      <a class="nav-row" href={f.asset.browser_download_url}><span class="grow">{osNames[f.os]}: {f.label}</span><span class="val">{mb(f.asset.size)}</span></a>
    {/each}
    <a class="nav-row" href={RELEASES} target="_blank" rel="noreferrer"><span class="grow">Release notes</span><span class="val">GitHub</span></a>
  </div>
{/if}
<p class="list-foot">Push to talk while Den is in the background, screen share with system sound on Windows, and showing what you're playing.</p>

<style>
  .gap-top { margin-top: 18px; }
</style>
