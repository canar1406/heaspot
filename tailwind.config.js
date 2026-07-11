/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      fontFamily: {
        sans: ["Segoe UI Variable", "Segoe UI", "system-ui", "sans-serif"],
      },
      colors: {
        surface: {
          light: "rgba(250, 250, 250, 0.85)",
          dark: "rgba(24, 24, 27, 0.82)",
        },
      },
    },
  },
  plugins: [],
};
