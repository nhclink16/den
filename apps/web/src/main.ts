import { mount } from 'svelte'
import './app.css'
import './plugins/canvas'
import './plugins/terminal'
import App from './App.svelte'

// iOS draws :active only under a touch listener; one on the document gives every control its pressed look.
document.addEventListener('touchstart', () => {}, { passive: true })

export default mount(App, { target: document.getElementById('app')! })
