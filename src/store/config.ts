
import { writable, derived } from 'svelte/store'
import type { UserSettings } from '../types/store'

export const font_families = [
    { value: 'Monospace1', label: 'Monospace' },
    { value: 'SansSerif1', label: 'Sans-Serif 1' },
    { value: 'SansSerif2', label: 'Sans-Serif 2' },
    { value: 'Serif1', label: 'Serif' },
]

export const user_settings = writable<UserSettings>({
    md_editor_width: 50,
    is_light: false,
    font_family: 'monospace1',
    font_size: 15,
    startup_filepath: '',
})
