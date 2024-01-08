<header>
  <Header />
</header>
<main class="container">
  <Editor />
</main>
<footer>
  <Footer
    on:is_light_change={ is_light_on_change }
    on:font_family_change={ font_family_on_change }
    on:font_size_change={ font_size_on_change } />
</footer>

<script lang="ts">
// import { getContext } from 'svelte'
import { invoke } from "@tauri-apps/api/tauri"
import { getCurrent } from '@tauri-apps/api/window';

import Editor from './components/Editor.svelte'
import Header from './components/Header.svelte'
import Footer from './components/Footer.svelte'
import { font_families } from './store/config'

const FORM_FONT_SIZE_SCALE = 1.1

function is_light_on_change(e: CustomEvent<{is_light: boolean}>) {
  let class_list = document.documentElement.classList
  if (e.detail.is_light) {
    class_list.add('light')
  } else {
    class_list.remove('light')
  }
}

function font_family_on_change(e: CustomEvent<{font_family: string}>) {
  const CLASS_PREFIX = 'font-family-'
  let class_list = document.documentElement.classList
  font_families.forEach(x => {
    class_list.remove(CLASS_PREFIX + x.value)
  })
  const font_family = e.detail.font_family
  class_list.add(CLASS_PREFIX + font_family)
}

function font_size_on_change(e: CustomEvent<{font_size: number}>) {
  document.documentElement.style.setProperty('--theme-font-size', e.detail.font_size.toString() + 'px')
  document.documentElement.style.setProperty('--theme-form-font-size', (e.detail.font_size * FORM_FONT_SIZE_SCALE).toString() + 'px')
}

const app_window = getCurrent();
const app_window_on_close_requested = () => {
  const width = window.outerWidth;
  const height = window.outerHeight;
  const x = window.screenX;
  const y = window.screenY;
  invoke("update_window", { width: width, height: height, x: x, y: y })
  // invoke("update_md_content", { mdContent: getContext<any>('md_content').value.toString() })
}
app_window.onCloseRequested(app_window_on_close_requested)
</script>

<style>
footer {
  margin-top: 1.7rem;
}
</style>