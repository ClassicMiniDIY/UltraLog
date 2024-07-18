<script setup lang="ts">
  import { defineProps } from 'vue';
  import type { LogChannel } from '~/types';

  const props = defineProps({
    channel: {
      type: Object as PropType<LogChannel>,
      required: true,
    },
    index: {
      type: Number,
      required: true,
    },
  });
</script>

<template>
  <UDashboardCard :title="channel.name">
    <template #icon>
      <UIcon name="i-heroicons-chart-bar-20-solid" />
    </template>
    <template #description>
      <p class="text-sm text-gray-500">Channel ID: {{ channel.id }}</p>
      <p class="text-sm text-gray-500">Channel ID: {{ channel.type }}</p>
      <p class="text-sm text-gray-500">Channel ID: {{ channel.display_min }}/{{ channel.display_max }}</p>
    </template>
  </UDashboardCard>
</template>
