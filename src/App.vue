<script setup lang="ts">
  import type { LogChannel } from './types';
  import ChannelCard from './components/ChannelCard.vue';

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
  <div class="flex h-screen">
    <!-- Sidebar -->
    <div class="w-64 bg-gray-800 text-white">
      <div class="p-5">Sidebar Content</div>
    </div>
    <!-- Main Content -->
    <div class="flex-1 p-10">
      <div class="grid grid-cols-5 gap-4">
        <!-- Dashboard Widgets -->
        <ChannelCard></ChannelCard>
        <ChannelCard></ChannelCard>
        <ChannelCard></ChannelCard>
        <ChannelCard></ChannelCard>
        <ChannelCard></ChannelCard>
        <ChannelCard></ChannelCard>
        <ChannelCard></ChannelCard>
        <ChannelCard></ChannelCard>
        <ChannelCard></ChannelCard>
        <!-- Additional widgets can be added here -->
      </div>
      <div class="grid grid-cols-1 gap-4 pt-3">
        <div class="bg-white p-6 rounded-lg shadow">Widget 3</div>
      </div>
    </div>
  </div>
</template>
