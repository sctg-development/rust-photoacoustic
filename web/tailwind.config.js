import { heroui } from "@heroui/theme"

/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./index.html",
    './src/layouts/**/*.{js,ts,jsx,tsx,mdx}',
    './src/pages/**/*.{js,ts,jsx,tsx,mdx}',
    './src/components/**/*.{js,ts,jsx,tsx,mdx}',
    "./node_modules/@heroui/theme/dist/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      fontSize: {
        '5xs': ['0.375rem', { lineHeight: '0.5rem' }], // 6px
        '4xs': ['0.5rem', { lineHeight: '0.625rem' }], // 8px
        '3xs': ['0.625rem', { lineHeight: '0.75rem' }], // 10px
        'xxs': ['0.6875rem', { lineHeight: '0.875rem' }], // 11px
      },
      backgroundImage: {
        'gradient-border-violet': 'linear-gradient(hsl(var(--heroui-background)), hsl(var(--heroui-background))), linear-gradient(83.87deg, #F54180, #9353D3)',
      },
    },
  },
  darkMode: "class",
  plugins: [heroui()],
}
