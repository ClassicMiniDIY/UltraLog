<script setup lang="ts">
  // const footerLinks = [
  //   {
  //     label: 'Invite people',
  //     icon: 'i-heroicons-plus',
  //     to: '/settings/members',
  //   },
  //   {
  //     label: 'Help & Support',
  //     icon: 'i-heroicons-question-mark-circle',
  //     click: () => (isHelpSlideoverOpen.value = true),
  //   },
  // ];
</script>

<template>
  <UDashboardLayout>
    <UDashboardPanel :width="250" :resizable="{ min: 200, max: 300 }" collapsible>
      <UDashboardNavbar class="!border-transparent" :ui="{ left: 'flex-1' }">
        <template #left>
          <SideBarHeader />
        </template>
      </UDashboardNavbar>

      <UDashboardSidebar>
        <template #header>
          <!-- <UDashboardSearchButton /> -->
        </template>
        <!--
        <UDashboardSidebarLinks :links="links" /> -->

        <!-- <UDivider />

        <UDashboardSidebarLinks
          :links="[{ label: 'Colors', draggable: true, children: colors }]"
          @update:links="(colors) => (defaultColors = colors)"
        />

        <div class="flex-1" />

        <UDashboardSidebarLinks :links="footerLinks" /> -->

        <UDivider class="sticky bottom-0" />

        <template #footer>
          <!-- ~/components/UserDropdown.vue -->
          <!-- <UserDropdown /> -->
        </template>
      </UDashboardSidebar>
    </UDashboardPanel>

    <slot />

    <!-- ~/components/HelpSlideover.vue -->
    <HelpSlideover />
  </UDashboardLayout>
</template>
