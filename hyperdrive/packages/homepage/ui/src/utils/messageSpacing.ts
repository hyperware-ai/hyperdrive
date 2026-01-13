const TIME_THRESHOLD_WIDE = 15 * 60; // 15 minutes in seconds

export type SpacingClass = 'tight' | 'wide';

interface SpacingInput {
  currentTimestamp: number;
  currentSender: string;
  prevTimestamp?: number;
  prevSender?: string;
  isNewDate: boolean;
}

export function getMessageSpacing(input: SpacingInput): SpacingClass {
  const { currentTimestamp, currentSender, prevTimestamp, prevSender, isNewDate } = input;

  // First message or new date = wide
  if (!prevTimestamp || !prevSender || isNewDate) {
    return 'wide';
  }

  // Sender changed = wide
  if (currentSender !== prevSender) {
    return 'wide';
  }

  // 15+ minutes elapsed = wide
  const timeDiff = currentTimestamp - prevTimestamp;
  if (timeDiff >= TIME_THRESHOLD_WIDE) {
    return 'wide';
  }

  return 'tight';
}

// Helper to check if dates are different days
export function isDifferentDay(ts1: number, ts2: number): boolean {
  const d1 = new Date(ts1 * 1000);
  const d2 = new Date(ts2 * 1000);
  return d1.toDateString() !== d2.toDateString();
}
