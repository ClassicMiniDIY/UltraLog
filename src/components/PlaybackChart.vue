<script lang="ts" setup>
  import type { LogChannel } from '../types';
  import { PropType } from 'vue';
  import { Line } from 'vue-chartjs';
  import {
    Chart as ChartJS,
    CategoryScale,
    LinearScale,
    PointElement,
    LineElement,
    Title,
    Tooltip,
    Legend,
    ChartOptions,
  } from 'chart.js';
  import { GraphColors } from '../types/constants';
  const props = defineProps({
    channels: {
      type: Object as PropType<LogChannel[]>,
      required: true,
      default: [],
    },
  });

  const data = {
    labels: ['January', 'February', 'March', 'April', 'May', 'June', 'July'],
    datasets: [
      {
        label: 'Data One',
        borderColor: GraphColors.ONE,
        backgroundColor: GraphColors.ONE,
        data: [40, 39, 10, 40, 39, 80, 40],
        yAxisID: 'y',
      },
      {
        label: 'Data Two',
        borderColor: GraphColors.TWO,
        backgroundColor: GraphColors.TWO,
        data: [60, 55, 32, 10, 2, 12, 53],
        yAxisID: 'y1',
      },
      {
        label: 'Data Three',
        borderColor: GraphColors.THREE,
        backgroundColor: GraphColors.THREE,
        data: [28, 48, 40, 19, 78, 31, 85],
        yAxisID: 'y2',
      },
    ],
  };
  props.channels.forEach((channel) => {
    console.log(channel);
  });

  const options: ChartOptions = {
    responsive: true,
    interaction: {
      mode: 'index',
      intersect: false,
    },
    plugins: {
      title: {
        display: true,
        text: 'Chart.js Line Chart - Multi Axis',
      },
    },
    scales: {
      y: {
        type: 'linear',
        display: false,
        position: 'left',
      },
      y1: {
        type: 'linear',
        display: false,
        position: 'left',
        // grid line settings
        grid: {
          drawOnChartArea: false, // only want the grid lines for one axis to show up
        },
      },
      y2: {
        type: 'linear',
        display: false,
        position: 'left',
        // grid line settings
        grid: {
          drawOnChartArea: false, // only want the grid lines for one axis to show up
        },
      },
    },
  };
</script>

<script lang="ts">
  ChartJS.register(CategoryScale, LinearScale, PointElement, LineElement, Title, Tooltip, Legend);
</script>
<template>
  <div class="card bg-base-300 shadow-xl">
    <div class="card-body">
      <Line :data="data" :options="options" />
    </div>
  </div>
</template>
