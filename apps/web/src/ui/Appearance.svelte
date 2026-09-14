<script lang="ts">
  import { onDestroy } from 'svelte'
  import { themes, fontFamilies, validateTheme, validName } from '../lib/theme.svelte'
  import type { Theme, ThemeColors, ThemeFonts } from '../lib/types'
  let editor = $state(false)
  let naming = $state(false)
  let name = $state('')
  let message = $state('')
  let fileInput: HTMLInputElement
  const active = $derived(themes.active)
  const roles: (keyof ThemeColors)[] = ['bg','bg2','bg3','line','ink','ink2','ink3','accent','success','danger']
  const fontRoles: (keyof ThemeFonts)[] = ['display','body','mono']
  const chosen = (t: Theme) => (themes.appearance.mode === 'system' || themes.appearance.mode === t.appearance) && t.id === (t.appearance === 'light' ? themes.appearance.light_theme : themes.appearance.dark_theme)
  function change(patch: Partial<Theme>) { message = ''; themes.preview({ ...JSON.parse(JSON.stringify(active)), ...patch }) }
  function color(role: keyof ThemeColors, value: string) {
    if (/^#[0-9a-f]{6}$/i.test(value)) change({ colors: { ...active.colors, [role]: value } })
  }
  function font(role: keyof ThemeFonts, value: string) {
    if (validName(value)) change({ fonts: { ...active.fonts, [role]: value } })
    else themes.error = 'Font names must be 1–40 letters, numbers, spaces, hyphens or underscores.'
  }
  async function save(e: SubmitEvent) {
    e.preventDefault()
    try { await themes.add({ ...JSON.parse(JSON.stringify(active)), id: crypto.randomUUID(), name: name.trim() }); naming = false; message = 'Theme saved.' }
    catch (e) { themes.error = (e as Error).message }
  }
  function exportTheme() {
    const blob = new Blob([JSON.stringify(active, null, 2) + '\n'], { type: 'application/json' })
    const url = URL.createObjectURL(blob), a = document.createElement('a')
    a.href = url; a.download = `${active.name}.den-theme.json`; a.click(); setTimeout(() => URL.revokeObjectURL(url), 1000)
  }
  async function importTheme(e: Event) {
    const file = (e.target as HTMLInputElement).files?.[0]
    if (!file) return
    try {
      if (file.size > 16384) throw Error('Theme files must be at most 16 KiB.')
      const t = validateTheme(JSON.parse(await file.text()))
      if (themes.all.some(v => v.id === t.id)) t.id = crypto.randomUUID()
      await themes.add(t); message = 'Theme imported.'
    } catch (e) { themes.error = (e as Error).message }
    fileInput.value = ''
  }
  onDestroy(() => themes.reset())
</script>

<div class="appearance">
  <div class="intro"><h2 class="display">Appearance</h2><p class="muted">Themes sync to your account</p></div>
  <fieldset><legend>Mode</legend><div class="segments">
    {#each ['light','dark','system'] as mode}<button aria-pressed={themes.appearance.mode === mode} onclick={() => themes.save({ ...themes.appearance, mode: mode as 'light' | 'dark' | 'system' })}>{mode[0]!.toUpperCase() + mode.slice(1)}</button>{/each}
  </div>{#if themes.appearance.mode === 'system'}<p class="hint muted">Follows your device</p>{/if}</fieldset>
  <div class="workspace" class:editing={editor}>
    <div class="gallery">
      {#each ['light','dark'] as appearance}
        <h3>{appearance === 'light' ? 'Light themes' : 'Dark themes'}</h3>
        <div class="theme-grid">
          {#each themes.all.filter(t => t.appearance === appearance) as t (t.id)}
            <div class="card-wrap">
              <button class="theme-card" class:chosen={chosen(t)} aria-label={`Use ${t.name} theme`} aria-pressed={chosen(t)} onclick={() => { themes.select(t); message = '' }}
                style={`--preview-bg:${t.colors.bg};--preview-bg2:${t.colors.bg2};--preview-bg3:${t.colors.bg3};--preview-line:${t.colors.line};--preview-ink:${t.colors.ink};--preview-ink2:${t.colors.ink2};--preview-accent:${t.colors.accent};--preview-display:"${t.fonts.display}"`}>
                <div class="mini" aria-hidden="true"><div class="mini-side"><b>den</b><i></i><i></i><i></i></div><div class="mini-main"><div class="mini-message"><span class="mini-avatar"></span><div><b>Evening, everyone</b><span class="mini-mention">@you</span></div></div><div class="mini-composer">Message…</div></div></div>
                <div class="caption"><span>{t.name}</span>{#if chosen(t)}<span aria-hidden="true">✓</span>{/if}</div>
              </button>
              {#if themes.appearance.custom_themes.some(c => c.id === t.id)}<div class="custom-label">custom <button aria-label={`Delete ${t.name} theme`} onclick={() => themes.remove(t.id)}>×</button></div>{/if}
            </div>
          {/each}
        </div>
      {/each}
      <div class="actions"><button class="btn" aria-expanded={editor} onclick={() => editor = !editor}>Customize</button><button class="btn" onclick={exportTheme}>Export</button><button class="btn" onclick={() => fileInput.click()}>Import</button><input class="sr-only" tabindex="-1" bind:this={fileInput} type="file" accept=".json,application/json" aria-label="Import theme file" onchange={importTheme} /></div>
    </div>
    {#if editor}<section class="editor" aria-label="Theme editor"><h3>Colors <small class="muted">Live preview</small></h3>
      {#each roles as role}<div class="color-row"><label for={`hex-${role}`}>{role}</label><input type="color" aria-label={`${role} color`} value={active.colors[role]} oninput={e => color(role, e.currentTarget.value)} /><input id={`hex-${role}`} class="field mono" aria-label={`${role} hex`} value={active.colors[role]} maxlength="7" pattern="#[0-9a-fA-F]{6}" oninput={e => color(role, e.currentTarget.value)} /></div>{/each}
    </section>{/if}
  </div>
  <section class="fonts"><h3>Fonts</h3><div class="font-grid">{#each fontRoles as role}<div class="font-role">
    <label><span class="role-name">{role}</span><select class="field" aria-label={`${role} font`} value={active.fonts[role]} onchange={e => font(role, e.currentTarget.value)}>{#each [...new Set([...fontFamilies, active.fonts[role]])] as family}<option style={`font-family:"${family}"`} value={family}>{family}</option>{/each}</select></label>
    <label class="hint muted">Use any Google Font<input class="field" aria-label={`Use any Google Font for ${role}`} placeholder="Family name" maxlength="40" onchange={e => font(role, e.currentTarget.value)} /></label>
  </div>{/each}</div></section>
  <div class="options"><fieldset><legend>Radius</legend><div class="segments">{#each ['sharp','soft','round'] as radius}<button aria-pressed={active.radius === radius} onclick={() => change({ radius: radius as Theme['radius'] })}>{radius[0]!.toUpperCase() + radius.slice(1)}</button>{/each}</div></fieldset>
  <fieldset><legend>Density</legend><div class="segments">{#each ['compact','comfortable'] as density}<button aria-pressed={active.density === density} onclick={() => change({ density: density as Theme['density'] })}>{density[0]!.toUpperCase() + density.slice(1)}</button>{/each}</div></fieldset></div>
  <div class="actions"><button class="btn" onclick={() => { naming = true; name = `${active.name} custom`.slice(0,40) }}>Save as new theme</button><button class="btn" onclick={() => { themes.reset(); message = ''; themes.error = '' }}>Reset</button>{#if themes.draft}<span class="hint muted">Live preview · Save to keep your changes</span>{/if}</div>
  {#if naming}<form class="save-name" onsubmit={save}><label>Theme name<input class="field" bind:value={name} maxlength="40" required /></label><button class="btn" type="submit">Save theme</button><button class="btn" type="button" onclick={() => naming = false}>Cancel</button></form>{/if}
  <p class="hint muted" role="status">{themes.saving ? 'Saving…' : message}</p><p class="error" role="alert">{themes.error}</p>
</div>

<style>
  .appearance { max-width: 1100px; padding-bottom: 30px; }
  h2 { font-size: 28px; margin: 0; } .intro p { margin: 4px 0 24px; }
  h3 { font-size: 15px; font-weight: 700; margin: 24px 0 10px; } h3 small { font-size: 12px; font-weight: 400; margin-inline-start: 12px; }
  fieldset { border: 0; padding: 0; margin: 0; min-width: 0; } legend { font-size: 14px; font-weight: 700; margin-bottom: 8px; }
  .segments { display: inline-flex; flex-wrap: wrap; padding: 3px; gap: 3px; background: var(--bg2); border: 1px solid var(--line); border-radius: var(--r); }
  .segments button { min-height: 36px; padding: 6px 14px; border-radius: var(--r); color: var(--ink2); }
  .segments button[aria-pressed=true] { background: var(--bg3); color: var(--ink); box-shadow: 0 0 0 1px var(--line); font-weight: 700; }
  .hint { font-size: 12px; margin: 6px 0 0; } .workspace { display: grid; gap: 28px; align-items: start; } .workspace.editing { grid-template-columns: minmax(180px,1fr) 260px; }
  .theme-grid { display: grid; grid-template-columns: repeat(auto-fill,180px); gap: 14px; }
  .theme-card { width: 180px; text-align: start; border: 1px solid var(--line); border-radius: var(--r-lg); overflow: hidden; color: var(--ink); }
  .theme-card:hover { border-color: var(--ink2); } .theme-card.chosen { outline: 2px solid var(--ink2); outline-offset: 2px; }
  .mini { height: 98px; display: flex; background: var(--preview-bg); color: var(--preview-ink); font-family: system-ui,sans-serif; }
  .mini-side { width: 42px; background: var(--preview-bg2); padding: 9px 7px; border-inline-end: 1px solid var(--preview-line); }
  .mini-side b { font-size: 10px; } .mini-side i { display: block; height: 3px; background: var(--preview-line); margin-top: 9px; border-radius: 2px; }
  .mini-main { flex: 1; padding: 15px 8px 8px; min-width: 0; display: flex; flex-direction: column; justify-content: space-between; }
  .mini-message { display: flex; gap: 5px; align-items: start; font-size: 7px; } .mini-avatar { width: 15px; height: 15px; border-radius: 50%; background: var(--preview-bg3); flex-shrink: 0; }
  .mini-message b { display: block; margin-bottom: 4px; } .mini-mention { color: var(--preview-accent); background: color-mix(in srgb,var(--preview-accent) 18%,transparent); }
  .mini-composer { background: var(--preview-bg3); border: 1px solid var(--preview-line); color: var(--preview-ink2); padding: 5px; border-radius: 3px; font-size: 7px; }
  .caption { display: flex; justify-content: space-between; align-items: center; padding: 9px 11px; font-family: var(--preview-display),sans-serif; background: var(--bg2); font-size: 15px; }
  .card-wrap { position: relative; } .custom-label { display: flex; align-items: center; justify-content: space-between; font: 11px var(--mono); color: var(--ink2); padding: 3px 8px; }
  .custom-label button { font: 20px var(--body); width: 28px; height: 28px; opacity: 0; } .card-wrap:hover .custom-label button,.card-wrap:focus-within .custom-label button { opacity: 1; }
  .editor { border-inline-start: 1px solid var(--line); padding-inline-start: 20px; } .color-row { display: grid; grid-template-columns: 1fr 32px 96px; align-items: center; gap: 8px; margin: 10px 0; font-size: 13px; }
  .color-row input[type=color] { width: 32px; height: 34px; padding: 2px; border: 1px solid var(--line); background: var(--bg2); border-radius: var(--r); }
  .field { min-width: 0; width: 100%; } .font-grid { display: grid; grid-template-columns: repeat(3,minmax(0,1fr)); gap: 16px; } label { display: grid; gap: 6px; } .role-name { text-transform: capitalize; font-size: 13px; } .font-role > label + label { margin-top: 10px; }
  select { font-family: var(--body); } .options { display: flex; gap: 28px; flex-wrap: wrap; margin-top: 24px; } .actions { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; margin-top: 20px; }
  .save-name { display: flex; align-items: end; flex-wrap: wrap; gap: 10px; margin-top: 16px; } .error { color: var(--danger); font-size: 13px; } .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }
  @media (max-width: 1150px) { .workspace.editing { grid-template-columns: 1fr; } .editor { border-inline-start: 0; padding-inline-start: 0; max-width: 360px; } }
  @media (max-width: 650px) { .font-grid { grid-template-columns: 1fr; } .theme-grid { grid-template-columns: repeat(auto-fill,minmax(150px,1fr)); } .theme-card { width: 100%; } .custom-label button { opacity: 1; } .field { font-size: 16px; } .segments button { min-height: 40px; } }
</style>
