<script setup lang="ts">
import { onMounted, ref, useTemplateRef } from "vue";
import { debounce } from "vue-debounce";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { LogicalSize, Window } from "@tauri-apps/api/window";

import IconApp from '../icons/application.svg';
import IconFile from '../icons/file.svg';
import IconSearch from '../icons/web.svg';

const selected = ref(0);
const items = ref<{ name: string, path: string, custom: 'search' | false }[]>([])
const input = useTemplateRef('input');
const list = useTemplateRef('list');

const query = debounce((val: string) => {
  selected.value = 0;
  items.value = [];
  if (/(http(s|):\/\/|).+\.+/.test(val)) {
    items.value.push({
      name: val,
      path: "Your default browser",
      custom: 'search'
    })
  }

  update_size();
  invoke("search", { query: val });
}, 400);

listen<[string, string]>("search-result", (event) => {
  items.value = [...items.value, { name: event.payload[0], path: event.payload[1], custom: false, }]
  update_size();
})

const update_size = () => {
  let height = 60 + (80 * Math.min(items.value.length, 5));
  let window = Window.getCurrent();
  window.setSize(new LogicalSize(800, height || 60));
}

const open = (index: number) => {
  const { name, path, custom } = items.value[index];
  switch (custom) {
    case "search":
      invoke("open", { path: name });
      break;

    default:
      invoke("open", { path: path });
  }

  setTimeout(() => close(), 500);
}

const nav = (ev: KeyboardEvent) => {
  if (ev.code == 'Tab' || ev.code == 'Enter') return;
  ev.preventDefault();
  input.value?.focus();
}

const close = () => {
  console.log('closing');
  input.value!.value = '';
  items.value = [];
  setTimeout(() => invoke("close"), 0);
}

onMounted(() => {
  input.value?.focus();
  window.addEventListener('blur', () => close());
  document.addEventListener('keydown', ev => {
    if (ev.code == 'Escape') close();
    if (ev.code == 'ArrowUp') selected.value = selected.value == 0 ? items.value.length : selected.value - 1;
    if (ev.code == 'ArrowDown') selected.value = (selected.value + 1) % items.value.length;
    if (ev.code == 'Enter') open(selected.value);

    list.value?.children[selected.value]?.scrollIntoView({
      block: 'center'
    });
  })
});

</script>

<style>
@import "tailwindcss";
</style>

<template>
  <div class="w-screen h-screen grid place-items-center">
    <main class="w-full h-full relative overflow-clip">
      <input ref="input" autofocus type="text" @input="event => query((event.target! as HTMLInputElement).value)"
        :class="`w-full h-15 border border-gray-200 rounded-lg text-xl indent-4 outline-none bg-white ${items.length > 0 ? 'rounded-b-[0]' : ''}`"
        placeholder="Search">
      <ul ref="list" v-if="items.length > 0"
        class="absolute w-full border border-gray-200 rounded-lg rounded-t-[0] h-full px-2 py-2.5 flex flex-col space-y-2 select-none overflow-y-auto bg-white">
        <li v-for="(item, index) in items" :key="index" :tabindex="index + 1"
          @keydown="key => key.code === 'Enter' ? open(index) : nav(key)" @click="() => open(index)"
          :class="`w-full hover:bg-gray-100 px-2 py-2 rounded-lg border border-gray-200 cursor-pointer flex space-x-4 ${selected == index ? 'bg-gray-100' : ''}`">
          <img :src="item.custom == 'search' ? IconSearch : /\.{2,4}$/.test(item.path) ? IconFile : IconApp">
          <div class="overflow-clip">
            <h2 class="font-semibold -mb-2 truncate max-w-full">{{ item.name }}</h2>
            <span class="text-xs truncate max-w-full">{{ item.path }}</span>
          </div>
        </li>
      </ul>
    </main>
  </div>
</template>