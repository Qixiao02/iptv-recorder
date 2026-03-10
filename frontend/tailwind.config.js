export default {
  content: [
    './index.html',
    './src/**/*.{js,ts,jsx,tsx}',
  ],
  theme: {
    extend: {
      colors: {
        // Base colors
        'bg-base': '#0A0A0B',
        'bg-elevated': '#111113',
        'bg-card': '#18181B',
        'bg-panel': '#1F1F23',
        'bg-input': '#27272A',

        // Borders
        'border-subtle': '#27272A',
        'border-default': '#3F3F46',
        'border-focus': '#3B82F6',

        // Text
        'text-primary': '#FAFAFA',
        'text-secondary': '#A1A1AA',
        'text-tertiary': '#71717A',
        'text-disabled': '#52525B',

        // Brand
        'brand': {
          DEFAULT: '#3B82F6',
          hover: '#60A5FA',
          active: '#2563EB',
        },

        // Semantic
        'success': '#10B981',
        'warning': '#F59E0B',
        'error': '#EF4444',
        'recording': '#F43F5E',
      },
      fontFamily: {
        sans: ['Outfit', '-apple-system', 'BlinkMacSystemFont', 'Segoe UI', 'Roboto', 'sans-serif'],
        mono: ['JetBrains Mono', 'Fira Code', 'monospace'],
      },
      borderRadius: {
        'sm': '4px',
        'md': '8px',
        'lg': '12px',
        'xl': '16px',
      },
      boxShadow: {
        'glow-brand': '0 0 20px rgba(59, 130, 246, 0.25)',
        'glow-recording': '0 0 20px rgba(244, 63, 94, 0.4)',
      },
      animation: {
        'pulse-recording': 'pulse-recording 2s ease-in-out infinite',
        'shimmer': 'shimmer 2s linear infinite',
        'fade-in': 'fade-in 0.3s ease-out forwards',
        'slide-in': 'slide-in-right 0.3s ease-out forwards',
      },
      keyframes: {
        'pulse-recording': {
          '0%, 100%': { opacity: '1' },
          '50%': { opacity: '0.5' },
        },
        'shimmer': {
          '0%': { backgroundPosition: '-200% 0' },
          '100%': { backgroundPosition: '200% 0' },
        },
        'fade-in': {
          '0%': { opacity: '0', transform: 'translateY(10px)' },
          '100%': { opacity: '1', transform: 'translateY(0)' },
        },
        'slide-in-right': {
          '0%': { opacity: '0', transform: 'translateX(20px)' },
          '100%': { opacity: '1', transform: 'translateX(0)' },
        },
      },
    },
  },
  plugins: [],
}
