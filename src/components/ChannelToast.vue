<script lang="ts" setup>
  import { ref } from 'vue';

  const props = defineProps({
    errorType: {
      type: Object as PropType<LogChannel[]>,
      required: true,
      default: [],
    },
  });

  const notFound = ref(false);
  const maxReached = ref(false);
  const alreadyAdded = ref(false);
  const noChannel = ref(false);
</script>

<template>
  <div class="toast toast-end">
    <div v-if="notFound" class="alert alert-warning">
      <span>Channel not found</span>
    </div>
    <div v-if="maxReached" class="alert alert-warning">
      <span>Max channels reached</span>
    </div>
    <div v-if="alreadyAdded" class="alert alert-warning">
      <span>Channel already added</span>
    </div>
    <div v-if="noChannel" class="alert alert-warning">
      <span>No channel selected</span>
    </div>
  </div>
</template>
