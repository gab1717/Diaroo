import { listen } from "@tauri-apps/api/event";

export interface MonitoringStatus {
  active: boolean;
  screenshots_today: number;
  last_batch_time: string | null;
}

export interface ClaudeOutput {
  text: string;
  done: boolean;
}

export function onMonitoringStatus(
  callback: (status: MonitoringStatus) => void,
) {
  return listen<MonitoringStatus>("monitoring-status", (event) => {
    callback(event.payload);
  });
}

export function onClaudeOutput(callback: (output: ClaudeOutput) => void) {
  return listen<ClaudeOutput>("claude-output", (event) => {
    callback(event.payload);
  });
}

export function onDigestReady(callback: (path: string) => void) {
  return listen<string>("digest-ready", (event) => {
    callback(event.payload);
  });
}

export function onPetSizeChanged(callback: (scale: number) => void) {
  return listen<number>("pet-size-changed", (event) => {
    callback(event.payload);
  });
}

export function onPetChanged(callback: (petName: string) => void) {
  return listen<string>("pet-changed", (event) => {
    callback(event.payload);
  });
}

export interface ActivityTick {
  app_name: string;
  window_title: string;
  hash_distance: number;
  was_skipped: boolean;
}

export function onActivityTick(callback: (tick: ActivityTick) => void) {
  return listen<ActivityTick>("activity-tick", (event) => {
    callback(event.payload);
  });
}

export function onWanderToggled(callback: (enabled: boolean) => void) {
  return listen<boolean>("wander-toggled", (event) => {
    callback(event.payload);
  });
}
