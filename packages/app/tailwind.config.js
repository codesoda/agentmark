/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  // Light and dark follow the system setting.
  darkMode: "media",
  theme: {
    extend: {
      colors: {
        // The one accent: AgentMark indigo, #4f46e5 (Tailwind indigo-600).
        accent: {
          DEFAULT: "#4f46e5",
          hover: "#4338ca",
          soft: "#eef2ff",
        },
      },
      fontSize: {
        "2xs": ["11px", "14px"],
      },
    },
  },
  plugins: [],
};
