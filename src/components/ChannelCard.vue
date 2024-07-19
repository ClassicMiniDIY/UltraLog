<script lang="ts" setup>
  import { PropType } from 'vue';
  import { LogChannel } from '../types';

  defineProps({
    channel: {
      type: Object as PropType<LogChannel>,
      required: true,
      default: {
        id: 0,
        name: '???',
        type: '???',
        display_min: 0,
        display_max: 100,
      },
    },
  });
</script>

<template>
  <div class="card bg-base-300 shadow-xl h-fit justify-normal">
    <div class="card-body p-3">
      <div class="card-title justify-between">
        <h2 class="card-title text-lg">{{ channel?.name }}</h2>
        <button class="btn btn-square btn-accent btn-xs">
          <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>
      <div class="card-actions justify-end"></div>
      <div class="grid grid-cols-2 gap-1 text-sm">
        <p>Type: {{ channel?.type }}</p>
        <p class="text-right">ID: {{ channel?.id }}</p>
        <p class="">Min: {{ channel?.display_min }}</p>
        <p class="text-right">Max: {{ channel?.display_max }}</p>
      </div>
    </div>
  </div>
</template>
