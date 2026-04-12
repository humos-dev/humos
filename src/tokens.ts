// Design tokens for humOS
// Single source of truth for colors, spacing, typography, and radii.

export const colors = {
  bg: '#080808',
  bg1: '#0b0b0b',
  bg2: '#0d0d0d',
  surface: '#111111',
  border: '#1a1a1a',
  border2: '#262626',
  text: '#e8e8e8',
  text1: '#bdbdbd',
  text2: '#999999',
  text3: '#777777',
  signal: '#3ecf8e',
  signalDim: 'rgba(62, 207, 142, 0.14)',
  error: '#f87171',
} as const;

export const spacing = {
  xs: 4,
  sm: 8,
  md: 16,
  lg: 24,
  xl: 32,
  xxl: 48,
} as const;

export const fontSize = {
  xs: '10px',
  sm: '11px',
  md: '12px',
  lg: '13px',
  xl: '14px',
  heading: '16px',
} as const;

export const radius = {
  sm: '4px',
  md: '6px',
  lg: '8px',
} as const;

export const fontFamily = {
  system: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
  mono: '"SF Mono", "JetBrains Mono", ui-monospace, monospace',
} as const;
