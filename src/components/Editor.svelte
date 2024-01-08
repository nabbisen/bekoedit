<!-- todo -->
{#if false}
{#each ['horizontal', 'vertical', 'markdown', 'html', 'manual', 'auto'] as caption, i}
  <label>
    <input type="radio" bind:group={ view } value={ caption } on:change={ view_on_change }>
    { caption }
  </label>
{/each}
{/if}
<input type="range" min=1 max=99 style="width: 100%; font-size: 2rem;" bind:value={ md_editor_width } on:change={ md_editor_width_on_change }>
<!-- todo -->
<div class="editors" role="region"
  on:dragenter={ handle_drag_enter }
  on:dragleave={ handle_drag_leave }
  on:dragend={ handle_drag_leave } >
  <!-- todo -->
  <div class="md-edit editor-wrapper" style="width: { md_editor_width }%;">
    <textarea class="editor" on:change="{ md_edit_on_change }" bind:value={ md_edit }></textarea>
  </div>
  <!-- todo -->
  <div class="html-edit editor-wrapper" style="width: { 100 - md_editor_width }%;">
      <div class="editor" bind:this={ html_edit }></div>
    <div id="td-contextmenu">
      <button on:click={ row_add_on_click }>row add</button>
      <button on:click={ row_delete_on_click }>row delete</button>
      <button on:click={ column_add_on_click }>column add</button>
      <button on:click={ column_delete_on_click }>column delete</button>
    </div>
  </div>
  {#if is_file_droppable }
    <!-- todo -->
    <div style="position: fixed; top: 0; left: 0; width: 100vw; height: 100vh; display: flex; justify-content: center; align-items: center; background: #191919; opacity: 0.7;">
      Drop Markdown or HTML
    </div>
  {/if}
</div>

<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from "@tauri-apps/api/tauri"
  import { listen } from '@tauri-apps/api/event'
  import { extname } from '@tauri-apps/api/path'
  import { message } from '@tauri-apps/api/dialog';
  import { user_settings } from "../store/config";

  // todo integrate with footer
  $: $user_settings

  let md_editor_width = 50;
  function md_editor_width_on_change(e: Event) {
    const md_editor_width = Number((e.currentTarget! as HTMLInputElement).value)
    invoke('update_md_editor_width', { mdEditorWidth: md_editor_width })
  }

  let md_edit = ''

  let html_edit = document.createElement('div')
  function md_edit_on_change() {
    reflect2html()
  }
  function reflect2html() {
    invoke<string>("md2html", { markdown: md_edit }).then((result: string) => {
      html_edit.innerHTML = result
      html_edit.querySelectorAll('*[contenteditable]').forEach(x => {
        x.addEventListener('blur', html_edit_on_change)
      })
      html_edit.querySelectorAll('td[contenteditable]').forEach(x => {
        x.addEventListener('contextmenu', td_on_contextmenu)
      })
    })
  }

  let view = 'horizontal'
  function view_on_change(e: Event) {
    console.log((e.currentTarget! as HTMLSelectElement).value)
  }

  function get_td_contextmenu(): HTMLElement {
    return document.getElementById('td-contextmenu')!
  }
  function show_td_contextmenu() {
    get_td_contextmenu().classList.add('show')
  }
  function hide_td_contextmenu() {
    get_td_contextmenu().classList.remove('show')
  }
  const CONTEXTMENU_KEY = {
    TABLE: 'data-table',
    TR: 'data-tr',
    TD: 'data-td',
  }
  function td_on_contextmenu(e: Event) {
    if (!get_td_contextmenu().classList.contains('show')) {
      e.preventDefault()
    }
    e.stopPropagation()

    const td = e.currentTarget as HTMLSelectElement
    const td_index = Array.from(td.parentElement!.children).indexOf(td)
    const tr = td.closest('tr')!
    const tr_index = Array.from(tr.parentElement!.children).indexOf(tr)
    const table = tr.closest('table')!
    const fn = (uuid: string) => {
      table.setAttribute('data-id', uuid)

      let contextmenu = get_td_contextmenu()
      contextmenu.setAttribute(CONTEXTMENU_KEY.TABLE, uuid)
      contextmenu.setAttribute(CONTEXTMENU_KEY.TR, tr_index.toString())
      contextmenu.setAttribute(CONTEXTMENU_KEY.TD, td_index.toString())
      show_td_contextmenu()
    }
    invoke<string>("uuid").then(fn)
  }
  function html_edit_on_change() {
    reflect2md()
  }

  function reflect2md() {
    invoke<string>("html2md", { html: html_edit.innerHTML }).then((result: string) => {
      md_edit = result
    })
  }

  function get_table(): HTMLTableElement {
    const table_id = get_td_contextmenu().getAttribute(CONTEXTMENU_KEY.TABLE)!.toString()
    const ret = document.querySelector('table[data-id="' + table_id + '"]') as HTMLTableElement
    return ret
  }
  function get_table_thead(): HTMLTableSectionElement {
    return get_table().querySelector('thead')!
  }
  function get_table_tbody(): HTMLTableSectionElement {
    return get_table().querySelector('tbody')!
  }
  function row_add_on_click() {
    let tbody = get_table_tbody()
    let tr: HTMLElement = tbody.querySelector('tr')!.cloneNode(true) as HTMLElement
    tr.querySelectorAll('td').forEach(x => {
      x.innerText = '(enter)'
      x.addEventListener('contextmenu', td_on_contextmenu)
    })
    const tr_index = Number(get_td_contextmenu().getAttribute(CONTEXTMENU_KEY.TR))
    tbody.querySelectorAll('tr')[tr_index].insertAdjacentElement('afterend', tr)

    reflect2md()
  }
  function row_delete_on_click() {
    get_table_tbody().querySelector('tr:last-child')!.remove()

    reflect2md()
  }
  function column_add_on_click() {
    let th_tr: Array<HTMLTableRowElement> = Array.from(get_table_thead().querySelectorAll('tr'))
    let td_trs: Array<HTMLTableRowElement> = Array.from(get_table_tbody().querySelectorAll('tr'))
    let trs = th_tr.concat(td_trs)
    trs.forEach((x, i) => {
      const is_th = i === 0
      let td = document.createElement(is_th ? 'th' : 'td')
      td.innerText = '(enter)'
      td.setAttribute('contenteditable', 'true')
      td.addEventListener('contextmenu', td_on_contextmenu)
      x.appendChild(td)
    })
    
    reflect2md()
  }
  function column_delete_on_click() {
    get_table_thead().querySelector('tr')!.removeChild(get_table_thead().querySelector('th:last-child')!)

    get_table_tbody().querySelectorAll('tr').forEach(x => {
      x.removeChild(x.querySelector('td:last-child')!)
    })

    reflect2md()
  }

  let is_file_droppable = false
  async function handle_drag_enter() {
    if (!is_file_droppable) is_file_droppable = true
  }
  async function handle_drag_leave() {
    if (is_file_droppable) is_file_droppable = false
  }
  async function handle_file_drop(e: any) {
    if (!is_file_droppable) return

    const filepaths = e.payload
    if (1 < filepaths.length) {
      message('Single file, please.')
      return
    }

    const filepath = filepaths[0]
    reflect_dropped(filepath)
  }
  async function reflect_dropped(filepath: string) {
    const ext = (await extname(filepath)).toLocaleLowerCase()
    const allowed_exts = ['md', 'markdown', 'html']
    const is_readable = allowed_exts.includes(ext)
    if (!is_readable) {
      message('Markdown / HTML file only.')
      return
    }
    const is_html = ext === 'html'
    invoke<string>("read_textfile", { filepath: filepath, isHtml: is_html }).then((result: string) => {
      if (!result) return

      switch (ext) {
        case "html": {
          html_edit.innerHTML = result
          reflect2md()
          break
        }
        default: {
          md_edit = result
          reflect2html()
        }
      }
    })
  }
  let _unlisten: CallableFunction;
  async function handle_document_click(e: MouseEvent) {
    if ((e.target as Element).matches('td[contenteditable]')) return

    hide_td_contextmenu()
  }
  async function ready() {
    document.addEventListener('click', handle_document_click)

    _unlisten = await listen('tauri://file-drop', handle_file_drop)

    // todo integrate with footer
    invoke('user_settings').then((result: any) => {
        const user_settings = JSON.parse(result.toString());
        md_editor_width = user_settings['md_editor_width']
        const filepath = user_settings['startup_filepath']
        if (filepath) {
          reflect_dropped(filepath)
        }
    })
  }
  onMount(ready)
</script>

<style>
  .editors {
    display: flex;
    flex-direction: row;
  }
  .editor-wrapper {
    min-height: 75vh;
    margin: 0.4rem 1.1rem;
  }
  .editor {
    width: 100%;
    height: 100%;
    opacity: 0.8;
  }
  
  :global(#td-contextmenu) {
    display: none;
  }
  :global(#td-contextmenu.show) {
    display: block;
  }
</style>