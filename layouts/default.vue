<script setup lang="ts">
  import type { ECU_TYPES } from '~/types';
  const { isHelpSlideoverOpen } = useDashboard();
  // const { isDashboardSearchModalOpen } = useUIState();
  // const { metaSymbol } = useShortcuts();
  const links = [
    // {
    //   label: 'Command menu',
    //   icon: 'i-heroicons-command-line',
    //   shortcuts: [metaSymbol.value, 'K'],
    //   click: () => {
    //     isDashboardSearchModalOpen.value = true;
    //   },
    // },
    {
      label: 'Documentation',
      icon: 'i-heroicons-book-open',
      to: 'https://ui.nuxt.com/pro/getting-started',
      target: '_blank',
    },
    {
      label: 'GitHub Source',
      icon: 'i-simple-icons-github',
      to: 'https://github.com/nuxt-ui-pro/dashboard',
      target: '_blank',
    },
    {
      label: 'Help & Support',
      icon: 'i-heroicons-question-mark-circle',
      shortcuts: ['?'],
      click: () => (isHelpSlideoverOpen.value = true),
    },
  ];
  const avatar = {
    label: 'Nuxt',
    avatar: {
      src: 'https://avatars.githubusercontent.com/u/23360933?s=200&v=4',
    },
  };
</script>

<template>
  <UDashboardLayout>
    <UDashboardPanel :width="250" :resizable="{ min: 200, max: 300 }" collapsible>
      <UDashboardNavbar class="!border-transparent" :ui="{ left: 'flex-1' }">
        <template #left>
          <UIcon name="i-heroicons-chart-bar" class="pt-2" />
          <!-- <UAvatar :src="avatar.src" size="2xs" /> -->
          <span class="truncate text-gray-900 dark:text-white font-semibold">Ultra Log Viewer</span>
        </template>
      </UDashboardNavbar>

      <UDashboardSidebar>
        <template #header>
          <h3>Your Logs</h3>
        </template>

        <UDivider />

        <SidebarCsvItem fileName="HaltechLog.csv" ecu="Haltech" />
        <SidebarCsvItem fileName="LinkLog.csv" ecu="Link" />

        <div class="flex-1" />

        <UDivider class="sticky bottom-0" />

        <template #footer>
          <UDashboardSidebarLinks :links="links" />
        </template>
      </UDashboardSidebar>
    </UDashboardPanel>
    <slot />
    <HelpSlideover />
  </UDashboardLayout>
</template>
