export type BehaviorState =
  | "idle"
  | "walk"
  | "sleep"
  | "run";

export interface TransitionContext {
  timeInState: number;
  activityLevel: number;
  switchRate: number;
  timeOfDay: number;
  isMonitoring: boolean;
  randomRoll: number;
}

export interface TransitionRule {
  to: BehaviorState;
  condition: (ctx: TransitionContext) => boolean;
}

export interface StateConfig {
  animation: string;
  minDuration: number;
  canMove: boolean;
  speed: number;
  interruptible: boolean;
  transitions: TransitionRule[];
}

export interface ActivityTickPayload {
  app_name: string;
  window_title: string;
  hash_distance: number;
  was_skipped: boolean;
  timestamp: number;
}
