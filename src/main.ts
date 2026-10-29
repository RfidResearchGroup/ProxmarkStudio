import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import { router } from './router'
import { useSettingsStore } from './stores/settings'
import '@xterm/xterm/css/xterm.css'
import './styles/tokens.css'
import './styles/base.css'

const app = createApp(App)
const pinia = createPinia()
app.use(pinia)
app.use(router)

const settings = useSettingsStore()
settings.hydrate()
settings.listenForSystemLocale()

app.mount('#app')
