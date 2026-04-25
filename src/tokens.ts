// Design tokens for humOS
// Single source of truth for colors, spacing, typography, and radii.
// Semantic rule: signal (green) = session health. coord (blue) = coordination. Never swap.

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
  signal: '#3ecf8e',     // session health only (running dot, waveform, brain ribbon)
  signalDim: 'rgba(62, 207, 142, 0.14)',
  coord: '#3b82f6',      // coordination only (pipe edges, pipe dot, signal broadcast)
  coordDim: 'rgba(59, 130, 246, 0.14)',
  amber: '#f59e0b',      // waiting status only
  error: '#f87171',
  gridLine: '#0e0e0e',   // background grid lines
} as const;

export const spacing = {
  xs: 4,
  sm: 8,
  md: 12,  // card padding (was 16 — compact density per DESIGN.md)
  lg: 16,
  xl: 24,
  xxl: 48,
} as const;

export const fontSize = {
  xs: '9px',
  sm: '10px',
  md: '11px',
  lg: '12px',
  xl: '13px',
  heading: '14px',
} as const;

export const radius = {
  sm: '3px',
  md: '5px',  // cards (was 6px — per DESIGN.md 5px for cards)
  lg: '6px',
} as const;

export const fontFamily = {
  mono: '"JetBrains Mono", ui-monospace, monospace',
} as const;
