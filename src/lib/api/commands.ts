import { invoke } from "@tauri-apps/api/core";
import type { PetInfo } from "../sprites/types";

export interface AppConfig {
  llm_provider: string;
  api_key: string;
  model: string;
  api_endpoint: string;
  screenshot_interval_secs: number;
  batch_interval_secs: number;
  dedup_threshold: number;
  data_dir: string;
  pet_name: string;
  pet_size: string;
  auto_report_enabled: boolean;
  auto_report_time: string;
  wander_enabled: boolean;
  pet_position_x: number | null;
  pet_position_y: number | null;
  auto_start_monitoring_time_enabled: boolean;
  auto_start_monitoring_time: string;
  launch_at_startup: boolean;
}

export async function startMonitoring(): Promise<void> {
  return invoke("start_monitoring");
}

export async function stopMonitoring(): Promise<void> {
  return invoke("stop_monitoring");
}

export async function generateDigest(date?: string): Promise<string> {
  return invoke("generate_digest", { date });
}

export async function getConfig(): Promise<AppConfig> {
  return invoke("get_config");
}

export async function setConfig(config: AppConfig): Promise<void> {
  return invoke("set_config", { config });
}

export async function runClaude(prompt: string): Promise<void> {
  return invoke("run_claude", { prompt });
}

export async function listPets(): Promise<PetInfo[]> {
  return invoke("list_pets");
}

export async function getPetInfo(name: string): Promise<PetInfo> {
  return invoke("get_pet_info", { name });
}

export async function installPet(path: string): Promise<PetInfo> {
  return invoke("install_pet", { path });
}

export async function removePet(name: string): Promise<void> {
  return invoke("remove_pet", { name });
}

export interface DateInfo {
  date: string;
  has_report: boolean;
}

export async function listDataDates(): Promise<DateInfo[]> {
  return invoke("list_data_dates");
}

export async function listReports(): Promise<string[]> {
  return invoke("list_reports");
}

export async function readReport(date: string): Promise<string> {
  return invoke("read_report", { date });
}

export async function openReportFile(date: string): Promise<void> {
  return invoke("open_report_file", { date });
}

export async function openPromptFile(): Promise<void> {
  return invoke("open_prompt_file");
}

export async function openExtractPromptFile(): Promise<void> {
  return invoke("open_extract_prompt_file");
}

export async function savePetPosition(x: number, y: number): Promise<void> {
  return invoke("save_pet_position", { x, y });
}
