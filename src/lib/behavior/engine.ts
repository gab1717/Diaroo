import type { BehaviorState, TransitionContext, ActivityTickPayload } from "./types";
import { STATE_CONFIGS } from "./states";
import { MovementController } from "./movement";

export interface BehaviorCallbacks {
  onAnimationChange: (animation: string, direction: 1 | -1) => void;
  onPositionChange: (x: number, y: number) => void;
  onStateChange?: (state: BehaviorState) => void;
}

export class BehaviorEngine {
  private state: BehaviorState = "idle";
  private stateStartTime = 0;
  private lastTransitionEval = 0;
  private readonly TRANSITION_INTERVAL = 500;

  private movement: MovementController;
  private callbacks: BehaviorCallbacks;
  private availableAnimations: Set<string>;

  // When entering a movement state, block movement until the caller
  // syncs the actual window position (prevents snap-back after drag).
  private positionSynced = true;

  // Wander toggle — when false, pet stays in place (no walk/run)
  private wanderEnabled = true;

  // Activity signals (populated by Phase 2)
  private activityLevel = 0;
  private switchRate = 0;
  private isMonitoring = false;

  constructor(
    callbacks: BehaviorCallbacks,
    screenWidth: number,
    screenHeight: number,
    petSize: number,
    startX: number,
    startY: number,
    availableAnimations: string[],
  ) {
    this.callbacks = callbacks;
    this.availableAnimations = new Set(availableAnimations);
    this.movement = new MovementController(
      screenWidth,
      screenHeight,
      petSize,
      startX,
      startY,
    );
    this.stateStartTime = performance.now();
  }

  get currentState(): BehaviorState {
    return this.state;
  }

  get position(): { x: number; y: number } {
    return { x: this.movement.x, y: this.movement.y };
  }

  get direction(): 1 | -1 {
    return this.movement.direction;
  }

  updateBounds(screenWidth: number, screenHeight: number, petSize: number) {
    this.movement.updateBounds(screenWidth, screenHeight, petSize);
  }

  /** Called every frame from the rAF loop */
  update(now: number, dt: number) {
    // Evaluate transitions (throttled)
    if (now - this.lastTransitionEval >= this.TRANSITION_INTERVAL) {
      this.lastTransitionEval = now;
      this.evaluateTransitions(now);
    }

    const config = STATE_CONFIGS[this.state];

    // Movement — blocked until position is synced from the real window
    if (config.canMove && config.speed > 0 && this.positionSynced) {
      const prevX = this.movement.x;
      const prevDir = this.movement.direction;
      this.movement.moveHorizontal(config.speed, dt);
      if (this.movement.direction !== prevDir) {
        this.callbacks.onAnimationChange(this.resolveAnimation(config.animation), this.movement.direction);
      }
      if (Math.round(prevX) !== Math.round(this.movement.x)) {
        this.callbacks.onPositionChange(this.movement.x, this.movement.y);
      }
    }
  }

  private evaluateTransitions(now: number) {
    const config = STATE_CONFIGS[this.state];
    const timeInState = now - this.stateStartTime;

    if (timeInState < config.minDuration) return;
    if (!config.interruptible) return;

    const ctx: TransitionContext = {
      timeInState,
      activityLevel: this.activityLevel,
      switchRate: this.switchRate,
      timeOfDay: new Date().getHours(),
      isMonitoring: this.isMonitoring,
      randomRoll: Math.random(),
    };

    for (const rule of config.transitions) {
      if (rule.condition(ctx)) {
        this.transitionTo(rule.to, now);
        return;
      }
    }
  }

  private transitionTo(newState: BehaviorState, now: number) {
    // Block movement states when wander is disabled
    if (!this.wanderEnabled && (newState === "walk" || newState === "run")) {
      return;
    }

    this.state = newState;
    this.stateStartTime = now;

    const config = STATE_CONFIGS[newState];
    if (config.canMove) {
      this.positionSynced = false;
    }
    const animation = this.resolveAnimation(config.animation);

    this.callbacks.onAnimationChange(animation, this.movement.direction);
    this.callbacks.onStateChange?.(newState);
  }

  /** Falls back to "idle" if the pet doesn't have the requested animation */
  private resolveAnimation(animation: string): string {
    if (this.availableAnimations.has(animation)) return animation;
    return "idle";
  }

  /** Sync internal position from actual window position. Unblocks movement. */
  syncPosition(x: number, y: number) {
    this.movement.syncFromWindow(x, y);
    this.positionSynced = true;
  }

  get needsPositionSync(): boolean {
    return !this.positionSynced;
  }


  // --- Activity signals (Phase 2) ---

  pushActivityTick(_payload: ActivityTickPayload) {
    // Will be implemented in Phase 2 via ActivitySignalAccumulator
  }

  setWanderEnabled(enabled: boolean) {
    this.wanderEnabled = enabled;
    // If currently walking/running, go back to idle
    if (!enabled && (this.state === "walk" || this.state === "run")) {
      this.transitionTo("idle", performance.now());
    }
  }

  setActivitySignals(activityLevel: number, switchRate: number, isMonitoring: boolean) {
    this.activityLevel = activityLevel;
    this.switchRate = switchRate;
    this.isMonitoring = isMonitoring;
  }

  /** Update available animations (e.g. when pet changes) */
  setAvailableAnimations(animations: string[]) {
    this.availableAnimations = new Set(animations);
  }

  /** Reset to idle state (e.g. when pet changes) */
  reset(startX: number, startY: number) {
    this.state = "idle";
    this.stateStartTime = performance.now();
    this.movement.syncFromWindow(startX, startY);
    this.callbacks.onAnimationChange("idle", this.movement.direction);
  }
}
