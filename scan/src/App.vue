<script setup lang="ts">
import { onMounted, ref, useTemplateRef } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { debounce } from "vue-debounce";
import { listen } from "@tauri-apps/api/event";

const selected = ref(0);
const items = ref<{ name: string, path: string }[]>([])
const input = useTemplateRef('input');

const query = debounce(val => {
  selected.value = 0;
  invoke("search", { query: val });
  items.value = [];
}, 400);

listen<[string, string]>("search-result", (event) => {
  items.value = [...items.value, { name: event.payload[0], path: event.payload[1] }]
})

const open = (index: number) => {
  invoke("open", { path: items.value[index].path });
  setTimeout(() => invoke("close"), 500);
}

const nav = (ev: KeyboardEvent) => {
  if (ev.code == 'Tab' || ev.code == 'Enter') return;

  ev.preventDefault();
  input.value?.focus();
}

onMounted(() => {
  window.addEventListener("blur", () => {
    invoke("close");
  });

  document.addEventListener('keydown', ev => {
    console.log(ev);

    if (ev.code == 'Escape') invoke("close");
    if (ev.code == 'ArrowUp') selected.value = selected.value == 0 ? items.value.length : selected.value - 1;
    if (ev.code == 'ArrowDown') selected.value = (selected.value + 1) % items.value.length;
    if (ev.code == 'Enter') open(selected.value);
  })
});

</script>

<style>
@import "tailwindcss";
</style>

<template>
  <div class="w-screen h-[calc(100vh-10rem)] grid place-items-center">
    <main class="w-xl relative">
      <input ref="input" autofocus type="text" @input="event => query((event.target! as HTMLInputElement).value)"
        class="w-full h-15 border border-gray-200 rounded-lg shadow-xl text-xl indent-4 outline-none bg-white"
        placeholder="Search">
      <ul v-if="items.length > 0"
        class="absolute mt-4 w-full border border-gray-200 rounded-lg shadow-sm max-h-[20rem] px-2 py-2.5 flex flex-col space-y-2 select-none overflow-y-auto bg-white">
        <li v-for="(item, index) in items" :key="index" :tabindex="index + 1"
          @keydown="key => key.code === 'Enter' ? open(index) : nav(key)" @click="() => open(index)"
          :class="`w-full hover:bg-gray-100 px-2 py-2 rounded-lg border border-gray-200 cursor-pointer ${selected == index ? 'bg-gray-100' : '' }`">
          <h2 class="font-semibold -mb-2">{{ item.name }}</h2>
          <span class="text-xs">{{ item.path }}</span>
        </li>
      </ul>
    </main>
  </div>
</template>