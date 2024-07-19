<template>
  <div class="navbar bg-base-300">
    <div class="navbar-start">
      <div class="drawer ml-5">
        <input id="my-drawer" type="checkbox" class="drawer-toggle" />
        <div class="drawer-content">
          <!-- Page content here -->
          <label for="my-drawer" class="btn btn-square btn-accent drawer-button"
            ><svg
              xmlns="http://www.w3.org/2000/svg"
              fill="none"
              viewBox="0 0 24 24"
              class="inline-block h-5 w-5 stroke-current"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M4 6h16M4 12h16M4 18h16"
              ></path></svg
          ></label>
        </div>
        <div class="drawer-side z-10">
          <label for="my-drawer" aria-label="close sidebar" class="drawer-overlay"></label>
          <ul class="menu bg-base-200 text-base-content min-h-full w-80 p-4">
            <!-- Sidebar content here -->
            <li><a>Sidebar Item 1</a></li>
            <li><a>Sidebar Item 2</a></li>
          </ul>
        </div>
      </div>
      <!-- <ThemeController /> -->
    </div>

    <div class="navbar-center">
      <h1 class="text-4xl text-primary font-logo">UltraLog</h1>
    </div>
    <div class="navbar-end"></div>
  </div>
</template>
