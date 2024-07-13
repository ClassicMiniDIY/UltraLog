<script setup lang="ts">
  import { defineProps } from 'vue';
  import type { ECU_TYPES } from '~/types';

  const props = defineProps({
    fileName: {
      type: String,
      required: true,
      default: 'HaltechLog.csv',
    },
    ecu: {
      type: String as PropType<ECU_TYPES>,
      required: true,
      default: 'Haltech',
    },
  });
</script>

<template>
  <UDashboardCard :title="fileName" icon="i-heroicons-document-text">
    <template #description>
      <div class="flex items">
        <UBadge v-if="ecu === 'Haltech'" color="yellow" variant="solid">Haltech</UBadge>
        <UBadge v-if="ecu === 'Link'" color="red" variant="solid">Link</UBadge>
      </div>
    </template>
    <template #footer>
      <div class="flex items-center justify-between">
        <span class="text-sm text-gray-500 dark:text-gray-400">Updated 2 days ago</span>
        <UIcon name="i-heroicons-chevron-right" class="w-4 h-4 text-gray-500 dark:text-gray-400" />
      </div>
    </template>
  </UDashboardCard>
</template>

<style lang="scss" scoped>
  .i-heroicons-document-text {
    width: 1.7rem !important;
    height: 1.7rem !important;
  }
</style>
