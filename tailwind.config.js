/** @type {import('tailwindcss').Config} */

import daisyui from 'daisyui';

export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx,vue}'],
  plugins: [daisyui],

  theme: {
    fontFamily: {
      logo: ['Fredoka One'],
    },
  },
  daisyui: {
    themes: [
      {
        UltraLog: {
          primary: '#71784E',
          secondary: '#F6F7EB',
          accent: '#BF4E30',
          neutral: '#292524',
          'base-100': '#292524',
          info: '#476C9B',
          success: '#9FA677',
          warning: '#FDC149',
          error: '#871E1C',
        },
      },
    ],
  },
};
