<script setup lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api';
  import html2canvas from 'html2canvas';
  import { sub } from 'date-fns';

  import type { Period, Range } from '~/types';

  const range = ref<Range>({ start: sub(new Date(), { days: 14 }), end: new Date() });
  const period = ref<Period>('daily');

  async function snapshot() {
    const element = document.getElementById('dashboardChart');
    console.log(element);

    const canvas = await html2canvas(element);
    const image = canvas.toDataURL('image/png');
    downloadImage(image, 'snapshot.png');
  }

  function downloadImage(dataUrl, filename) {
    const a = document.createElement('a');
    a.href = dataUrl;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
  }

  listen('tauri://file-drop', (event) => {
    const [filePath] = event.payload as string[];
    invoke('add_file', { filePath }).then((channels: any) => {
      console.log(JSON.parse(channels));
    });
  });
</script>

<template>
  <UDashboardPage>
    <UDashboardPanel grow>
      <UDashboardNavbar title="Log Playback Viewer">
        <template #right>
          <p class="mt-2">Version: 1.0.0</p>
        </template>
      </UDashboardNavbar>
      <UDashboardToolbar>
        <template #left>
          <PlaybackDateRangePicker v-model="range" class="-ml-2.5" />
          <PlaybackPeriodSelect v-model="period" :range="range" />
        </template>
        <template #right>
          <UButton label="Snapshot" color="gray" icon="i-heroicons-camera" @click="snapshot()"> </UButton>
        </template>
      </UDashboardToolbar>

      <UDashboardPanelContent>
        <div class="grid lg:grid-cols-4 lg:items-start gap-8 mt-3 mb-3">
          <CoreDataCard v-for="i in 8" />
        </div>
        <PlaybackChart :period="period" :range="range" />
      </UDashboardPanelContent>
    </UDashboardPanel>
  </UDashboardPage>
</template>
