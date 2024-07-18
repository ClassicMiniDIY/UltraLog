<template>
  <Line :data="data" :options="options" />
</template>

<script lang="ts">
  import {
    Chart as ChartJS,
    CategoryScale,
    LinearScale,
    PointElement,
    LineElement,
    Title,
    Tooltip,
    Legend,
  } from 'chart.js';
  import { Line } from 'vue-chartjs';

  ChartJS.register(CategoryScale, LinearScale, PointElement, LineElement, Title, Tooltip, Legend);
  export const data = {
    labels: ['January', 'February', 'March', 'April', 'May', 'June', 'July'],
    datasets: [
      {
        label: 'Data One',
        data: [40, 39, 10, 40, 39, 80, 40],
        borderColor: '#' + (0x1000000 + Math.random() * 0xffffff).toString(16).substr(1, 6),
        backgroundColor: '#' + (0x1000000 + Math.random() * 0xffffff).toString(16).substr(1, 6),
        yAxisID: 'y',
      },
      {
        label: 'Data Two',
        data: [60, 55, 32, 10, 2, 12, 53],
        borderColor: '#' + (0x1000000 + Math.random() * 0xffffff).toString(16).substr(1, 6),
        backgroundColor: '#' + (0x1000000 + Math.random() * 0xffffff).toString(16).substr(1, 6),
        yAxisID: 'y1',
      },
      {
        label: 'Data Three',
        data: [28, 48, 40, 19, 78, 31, 85],
        borderColor: '#' + (0x1000000 + Math.random() * 0xffffff).toString(16).substr(1, 6),
        backgroundColor: '#' + (0x1000000 + Math.random() * 0xffffff).toString(16).substr(1, 6),
        yAxisID: 'y2',
      },
    ],
  };

  export const options = {
    responsive: true,
    interaction: {
      mode: 'index',
      intersect: false,
    },
    stacked: false,
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

  export default {
    components: {
      Line,
    },
    data() {
      return {
        data,
        options,
      };
    },
  };
</script>
