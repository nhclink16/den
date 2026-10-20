import { mount } from 'svelte'
import './app.css'
import './plugins/canvas'
import './plugins/terminal'
import App from './App.svelte'

// iOS draws :active only under a touch listener; one on the document gives every control its pressed look.
document.addEventListener('touchstart', () => {}, { passive: true })
// Focus rings are for the keyboard (app.css): a touch or a mouse press hides them, and moving focus with a keyboard
// (Tab, the arrows through a radio group, Home or End) brings them back. Mouse too, because Chromium starts matching
// :focus-visible on a clicked button at any key press, even Shift or Cmd (Cmd+Shift+4, Cmd+Tab).
const root = document.documentElement
if (matchMedia('(pointer: coarse)').matches) root.dataset.input = 'touch'
addEventListener('pointerdown', (e) => { root.dataset.input = e.pointerType === 'touch' ? 'touch' : 'mouse' }, { capture: true, passive: true })
addEventListener('keydown', (e) => { if (/^(Tab|Arrow\w+|Home|End)$/.test(e.key)) delete root.dataset.input }, { capture: true })

export default mount(App, { target: document.getElementById('app')! })
