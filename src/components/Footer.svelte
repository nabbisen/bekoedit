<nav>
    <input id="toggle-menus" type="checkbox">
    <label for="toggle-menus">🌊</label>
    <div class="menus">
        <label>
            <input type="checkbox" bind:checked={ $user_settings.is_light }>
            Color
        </label>
        <select bind:value={ $user_settings.font_family }>
        {#each font_families as item, i}
            {#if i === 0}
                <option value={ item.value } selected>{ item.label }</option>
            {:else}
                <option value={ item.value }>{ item.label }</option>
            {/if}
        {/each}
        </select>
        <label>
            font size
            <input type="number" max="80" min="4" bind:value={ $user_settings.font_size }>
            px
        </label>
    </div>
</nav>

<script lang="ts">
import { createEventDispatcher, onMount } from "svelte";
import { invoke } from "@tauri-apps/api/tauri"
import { user_settings, font_families } from "../store/config";

const dispatch = createEventDispatcher()

$: $user_settings
let prev_user_settings = structuredClone($user_settings)
user_settings.subscribe(x => {
    if (x.is_light !== prev_user_settings.is_light) {
        invoke('update_is_light', { isLight: x.is_light }).then(() => {
            dispatch('is_light_change', { is_light: x.is_light });
        })
    }
    if (x.font_family !== prev_user_settings.font_family) {
        invoke('update_font_family', { fontFamily: x.font_family }).then(() => {
            dispatch('font_family_change', { font_family: x.font_family });
        })
    }
    if (x.font_size !== prev_user_settings.font_size) {
        invoke('update_font_size', { fontSize: x.font_size }).then(() => {
            dispatch('font_size_change', {font_size: x.font_size});
        })
    }
    prev_user_settings = structuredClone(x)
})

const ready = () => {
    invoke('user_settings').then((result: any) => {
        const user_settings = JSON.parse(result.toString());
        $user_settings.is_light = user_settings['color'] === 'Light'
        dispatch('is_light_change', { is_light: $user_settings.is_light });
        $user_settings.font_family = user_settings['font_family']
        dispatch('font_family_change', { font_family: $user_settings.font_family });
        let font_size = user_settings['font_size']
        $user_settings.font_size = 0 < font_size ? font_size : 15;
        dispatch('font_size_change', { font_size: $user_settings.font_size });
    })
}
onMount(ready)
</script>

<style>
#toggle-menus {
    display: none;
}
label[for="#toggle-menus"] {
    margin-left: 0.2rem;
    cursor: context-menu;
}
.menus {
    display: none;
}
#toggle-menus:checked ~ .menus {
    display: inline-flex;
}
</style>