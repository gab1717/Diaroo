import type { ActivityTickPayload } from "./types";

const WINDOW_MS = 60_000; // 60-second rolling window
const MAX_HASH_DISTANCE = 64; // dHash is 64-bit, max hamming distance is 64

export class ActivitySignalAccumulator {
  private ticks: ActivityTickPayload[] = [];

  push(tick: ActivityTickPayload) {
    this.ticks.push(tick);
    this.prune(tick.timestamp);
  }

  private prune(now: number) {
    const cutoff = now - WINDOW_MS;
    this.ticks = this.ticks.filter((t) => t.timestamp >= cutoff);
  }

  /** Normalized average hash distance (0.0 = no change, 1.0 = max change) */
  get activityLevel(): number {
    const withDistance = this.ticks.filter((t) => !t.was_skipped);
    if (withDistance.length === 0) return 0;
    const avg =
      withDistance.reduce((sum, t) => sum + t.hash_distance, 0) /
      withDistance.length;
    return Math.min(avg / MAX_HASH_DISTANCE, 1.0);
  }

  /** Number of distinct app switches per minute */
  get switchRate(): number {
    if (this.ticks.length < 2) return 0;

    let switches = 0;
    for (let i = 1; i < this.ticks.length; i++) {
      if (this.ticks[i].app_name !== this.ticks[i - 1].app_name) {
        switches++;
      }
    }

    // Extrapolate to per-minute rate based on actual window span
    const span = this.ticks[this.ticks.length - 1].timestamp - this.ticks[0].timestamp;
    if (span <= 0) return 0;
    return (switches / span) * 60_000;
  }

  /** Whether fresh data is arriving (at least 1 tick in the last 15s) */
  get isActive(): boolean {
    if (this.ticks.length === 0) return false;
    const latest = this.ticks[this.ticks.length - 1].timestamp;
    return Date.now() - latest < 15_000;
  }
}
