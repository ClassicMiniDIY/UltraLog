<script setup lang="ts">
  const avatar = {
    label: 'Nuxt',
    avatar: {
      src: 'https://avatars.githubusercontent.com/u/23360933?s=200&v=4',
    },
  };
</script>

<template>
  <UAvatar :src="avatar.src" size="2xs" />

  <span class="truncate text-gray-900 dark:text-white font-semibold">{{ avatar.label }}</span>
</template>
