import type { BehaviorState, StateConfig } from "./types";

export const STATE_CONFIGS: Record<BehaviorState, StateConfig> = {
  idle: {
    animation: "idle",
    minDuration: 2000,
    canMove: false,
    speed: 0,
    interruptible: true,
    transitions: [
      {
        to: "walk",
        condition: (ctx) => ctx.timeInState > 8000 && ctx.randomRoll < 0.15,
      },

      {
        to: "sleep",
        condition: (ctx) => {
          const isNight = ctx.timeOfDay >= 22 || ctx.timeOfDay < 6;
          const chance = isNight ? 0.30 : 0.10;
          return ctx.timeInState > 20000 && ctx.randomRoll < chance;
        },
      },
      {
        to: "run",
        condition: (ctx) => ctx.switchRate > 6,
      },
    ],
  },
  walk: {
    animation: "walk",
    minDuration: 3000,
    canMove: true,
    speed: 40,
    interruptible: true,
    transitions: [
      {
        to: "idle",
        condition: (ctx) => ctx.timeInState > 8000 && ctx.randomRoll < 0.20,
      },

    ],
  },
  sleep: {
    animation: "sleep",
    minDuration: 10000,
    canMove: false,
    speed: 0,
    interruptible: true,
    transitions: [
      {
        to: "idle",
        condition: (ctx) => ctx.activityLevel > 0.6 && ctx.timeInState > 15000,
      },
      {
        to: "idle",
        condition: (ctx) => ctx.timeInState > 30000 && ctx.randomRoll < 0.05,
      },
    ],
  },
  run: {
    animation: "run",
    minDuration: 2000,
    canMove: true,
    speed: 100,
    interruptible: true,
    transitions: [
      {
        to: "walk",
        condition: (ctx) => ctx.switchRate < 3 && ctx.timeInState > 3000,
      },
      {
        to: "idle",
        condition: (ctx) => ctx.timeInState > 6000 && ctx.randomRoll < 0.20,
      },
    ],
  },
};

export function getAvailableAnimations(): string[] {
  return [...new Set(Object.values(STATE_CONFIGS).map((c) => c.animation))];
}
