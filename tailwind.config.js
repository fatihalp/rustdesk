const gray = {
  50: '#fbfbfd', 100: '#f5f5f7', 200: '#e8e8ed', 300: '#d2d2d7', 400: '#a1a1a6',
  500: '#86868b', 600: '#6e6e73', 700: '#3a3a3c', 800: '#2c2c2e', 850: '#242426',
  900: '#1c1c1e', 950: '#000000',
};
const accent = {
  50: '#fbfbfd', 100: '#f5f5f7', 200: '#e8e8ed', 300: '#d2d2d7', 400: '#d2d2d7',
  500: '#f5f5f7', 600: '#f5f5f7', 700: '#d2d2d7', 800: '#a1a1a6', 900: '#6e6e73',
};

export default {
  content: [
    "./resources/views/**/*.blade.php",
    "./resources/portal/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      fontFamily: {
        sans: ['-apple-system', 'BlinkMacSystemFont', '"SF Pro Text"', 'Inter', 'system-ui', 'sans-serif'],
      },
      colors: {
        slate: gray,
        blue: accent,
        indigo: accent,
        emerald: { 300: '#e8e8ed', 400: '#d2d2d7', 500: '#f5f5f7', 600: '#d2d2d7' },
        amber: { 300: '#d2d2d7', 400: '#a1a1a6', 500: '#a1a1a6' },
        brand: accent,
      },
    },
  },
  plugins: [],
}
