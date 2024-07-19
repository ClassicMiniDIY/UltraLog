<script setup lang="ts">
  import type { LogChannel } from './types';
  import NavBar from './components/NavBar.vue';
  import ChannelCard from './components/ChannelCard.vue';
  import PlaybackChart from './components/PlaybackChart.vue';

  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api';
  import { ref } from 'vue';

  const channels = ref<LogChannel[]>([]);

  listen('tauri://file-drop', (event) => {
    const [filePath] = event.payload as string[];
    invoke('add_file', { filePath }).then((rawChannels: any) => {
      channels.value = JSON.parse(rawChannels);
      console.log(channels.value);
    });
  });
</script>

<template>
  <NavBar></NavBar>
  <!-- Sidebar -->
  <!-- <div class="w-64 bg-gray-800 text-white">
      <div class="p-5 text-xl">UltraLog</div>
    </div> -->
  <!-- Main Content -->

  <div class="p-5">
    <div class="grid grid-cols-5 gap-4">
      <div class="h-full col-span-4">
        <PlaybackChart></PlaybackChart>
      </div>
      <div class="grid grid-cols-subgrid gap-4">
        <ChannelCard></ChannelCard>
        <ChannelCard></ChannelCard>
      </div>
    </div>
  </div>

  <!-- <div class="flex-1 p-10">
      <div class="grid grid-cols-5 gap-4">
      </div>
      <div class="grid grid-cols-1 gap-4 pt-3">
        <PlaybackChart></PlaybackChart>
      </div>
    </div> -->
</template>
