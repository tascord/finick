<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { debounce } from "vue-debounce";
import { listen } from "@tauri-apps/api/event";

const index = ref(0);
const items = ref<{ name: string, path: string }[]>([])
const query = debounce(val => {
  invoke("search", { query: val })
  index.value = 0;
  items.value = [];
}, 400);

listen<[string, string]>("search-result", (event) => {
  items.value = [...items.value, { name: event.payload[0], path: event.payload[1] }]
})

</script>

<style>
@import "tailwindcss";
</style>

<template>
  <div class="w-screen h-[calc(100vh-10rem)] grid place-items-center">
    <main class="w-xl relative">
      <input autofocus type="text" v-model="query"
        class="w-full h-15 border border-gray-200 rounded-lg shadow-xl text-xl indent-4 outline-none"
        placeholder="Search">
      <ul v-if="items.length > 0"
        class="absolute mt-4 w-full border border-gray-200 rounded-lg shadow-sm max-h-[20rem] px-2 py-2.5 flex flex-col space-y-4 select-none overflow-y-auto">
        <li v-for="(item, index) in items" :key="index"
          :class="`w-full ${index == 0 ? 'bg-gray-100' : ''} px-2 py-2 rounded-lg border border-gray-200`">
          <h2 class="font-semibold -mb-2">{{ item.name }}</h2>
          <span class="text-xs">{{ item.path }}</span>
        </li>
      </ul>
    </main>
  </div>
</template>