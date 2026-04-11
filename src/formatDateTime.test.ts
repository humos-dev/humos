import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { formatDateTime } from './utils/formatDateTime';

describe('formatDateTime', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    // Fix "now" to 2024-04-12T12:00:00Z (a Friday)
    vi.setSystemTime(new Date('2024-04-12T12:00:00Z'));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('returns "Today" for a timestamp from today', () => {
    const iso = '2024-04-12T08:30:00Z';
    const { date, time } = formatDateTime(iso);
    expect(date).toBe('Today');
    expect(time).toBeTruthy();
  });

  it('returns "Yesterday" for a timestamp from yesterday', () => {
    const iso = '2024-04-11T15:00:00Z';
    const { date } = formatDateTime(iso);
    expect(date).toBe('Yesterday');
  });

  it('returns "3d ago" for a timestamp from 3 days ago', () => {
    const iso = '2024-04-09T10:00:00Z';
    const { date } = formatDateTime(iso);
    expect(date).toBe('3d ago');
  });

  it('returns "MMM D" format for a timestamp 7+ days ago', () => {
    const iso = '2024-04-05T10:00:00Z';
    const { date } = formatDateTime(iso);
    // toLocaleDateString en-US with month: short, day: numeric
    expect(date).toBe('Apr 5');
  });
});
