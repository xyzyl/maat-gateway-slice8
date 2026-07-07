/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      fontFamily: {
        sans: [
          "Inter Variable",
          "Inter",
          "system-ui",
          "-apple-system",
          "sans-serif",
        ],
        mono: [
          "JetBrains Mono Variable",
          "JetBrains Mono",
          "ui-monospace",
          "SFMono-Regular",
          "monospace",
        ],
      },
      colors: {
        // Surface tones — used for backgrounds and elevation.
        // Designed as a near-monochrome scale with cool undertones.
        ink: {
          50: "#f6f7f9",
          100: "#eceef2",
          200: "#d5d9e2",
          300: "#aab1c0",
          400: "#7c8497",
          500: "#5a6275",
          600: "#434a5d",
          700: "#323847",
          800: "#212635",
          900: "#161b27",
          950: "#0c1019",
        },
        // Single accent. Used sparingly: active nav, primary CTAs, focus rings.
        accent: {
          400: "#5dd8e4",
          500: "#22c1d3",
          600: "#0f9aab",
        },
        // Status colors. Intentionally muted — operators look at these all day.
        ok: {
          400: "#4ade80",
          500: "#22c55e",
          600: "#16a34a",
        },
        warn: {
          400: "#fbbf24",
          500: "#f59e0b",
          600: "#d97706",
        },
        bad: {
          400: "#f87171",
          500: "#ef4444",
          600: "#dc2626",
        },
      },
      boxShadow: {
        soft: "0 1px 0 rgba(255,255,255,0.04) inset, 0 1px 2px rgba(0,0,0,0.4)",
      },
    },
  },
  plugins: [],
};
