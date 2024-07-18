<script setup lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api';
  import { sub } from 'date-fns';
  import type { LogChannel, Period, Range } from '~/types';

  const range = ref<Range>({ start: sub(new Date(), { days: 14 }), end: new Date() });
  const period = ref<Period>('daily');
  const channels = ref<LogChannel[]>([]);
  const channelToAdd = ref<LogChannel | null>(null);
  const selectedChannels = ref<LogChannel[]>([]);

  listen('tauri://file-drop', (event) => {
    const [filePath] = event.payload as string[];
    invoke('add_file', { filePath }).then((rawChannels: any) => {
      channels.value = JSON.parse(rawChannels);
      console.log(channels.value);
    });
  });

  function addChannel() {
    console.log('add Channel', channelToAdd);

    if (channelToAdd.value) {
      selectedChannels.value.push(channelToAdd.value);
      channelToAdd.value = channels.value[1];
    }
  }
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
        <!-- <template #right>
          <UButton label="Snapshot" color="gray" icon="i-heroicons-camera" @click="snapshot()"> </UButton>
        </template> -->
      </UDashboardToolbar>
      <UDashboardToolbar>
        <template #left>
          <USelect
            option-attribute="name"
            v-model="channelToAdd"
            class="w-full"
            placeholder="Add a Channel"
            :options="channels"
          />
          <UButton
            icon="i-heroicons-plus"
            size="sm"
            color="primary"
            square
            variant="solid"
            @click="addChannel()"
            :disabled="selectedChannels.length > 9"
          />
        </template>
      </UDashboardToolbar>

      <UDashboardPanelContent>
        <div class="grid lg:grid-cols-4 lg:items-start gap-8 mt-3 mb-3">
          <template v-for="(channel, i) in selectedChannels">
            <CoreDataCard :channel="channel" :index="i" />
          </template>
        </div>
        <!-- <PlaybackChart :period="period" :range="range" :channels="selectedChannels" /> -->
      </UDashboardPanelContent>
    </UDashboardPanel>
  </UDashboardPage>
</template>
