<script lang="ts">
  import { onMount } from "svelte";
  import { getConfig, setConfig, openPromptFile, openExtractPromptFile, type AppConfig } from "./lib/api/commands";

  let config = $state<AppConfig>({
    llm_provider: "openrouter",
    api_key: "",
    model: "google/gemini-2.0-flash-001",
    api_endpoint: "",
    screenshot_interval_secs: 5,
    batch_interval_secs: 300,
    dedup_threshold: 5,
    data_dir: "",
    pet_name: "cat",
    pet_size: "medium",
    auto_report_enabled: false,
    auto_report_time: "17:00",
    wander_enabled: true,
    pet_position_x: null,
    pet_position_y: null,
    auto_start_monitoring_time_enabled: false,
    auto_start_monitoring_time: "09:00",
    launch_at_startup: false,
  });

  let statusMessage = $state("");

  onMount(async () => {
    try {
      config = await getConfig();
    } catch (e) {
      console.error("Failed to load config:", e);
    }
  });

  async function editPrompt() {
    try {
      await openPromptFile();
    } catch (e) {
      statusMessage = `Error: ${e}`;
    }
  }

  async function editExtractPrompt() {
    try {
      await openExtractPromptFile();
    } catch (e) {
      statusMessage = `Error: ${e}`;
    }
  }

  async function save() {
    try {
      await setConfig(config);
      statusMessage = "Settings saved!";
      setTimeout(() => (statusMessage = ""), 3000);
    } catch (e) {
      statusMessage = `Error: ${e}`;
    }
  }
</script>

<h1>Settings</h1>

<div class="form-group">
  <label for="provider">LLM Provider <span class="info-tooltip" data-tip="The AI service used to analyze your screenshots. OpenRouter provides access to many models, Ollama runs locally, Claude Code and Codex use their respective CLI tools.">i</span></label>
  <select id="provider" bind:value={config.llm_provider}>
    <option value="openrouter">OpenRouter</option>
    <option value="ollama">Ollama</option>
    <option value="claude-code">Claude Code</option>
    <option value="codex">Codex</option>
  </select>
</div>

{#if config.llm_provider === "claude-code"}
  <div class="form-group">
    <p class="provider-note">Uses the locally installed <code>claude</code> CLI. No API key or model selection needed.</p>
  </div>
{:else if config.llm_provider === "codex"}
  <div class="form-group">
    <p class="provider-note">Uses the locally installed <code>codex</code> CLI. No API key or model selection needed.</p>
  </div>
{:else if config.llm_provider === "ollama"}
  <div class="form-group">
    <label for="api-endpoint">API Endpoint <span class="info-tooltip" data-tip="The URL where your Ollama instance is running. Defaults to http://localhost:11434 if left empty.">i</span></label>
    <input id="api-endpoint" type="text" bind:value={config.api_endpoint} placeholder="http://localhost:11434" />
  </div>
{:else}
  <div class="form-group">
    <label for="apikey">API Key <span class="info-tooltip" data-tip="Your authentication key for the selected LLM provider. This is stored locally and never shared.">i</span></label>
    <input id="apikey" type="password" bind:value={config.api_key} placeholder="Enter API key..." />
  </div>
{/if}

{#if config.llm_provider !== "claude-code" && config.llm_provider !== "codex"}
  <div class="form-group">
    <label for="model">Model <span class="info-tooltip" data-tip="The specific AI model to use for analyzing screenshots. For OpenRouter, use the format 'provider/model-name'.">i</span></label>
    <input id="model" type="text" bind:value={config.model} />
  </div>
{/if}

<div class="form-group">
  <label for="interval">Screenshot Interval (seconds) <span class="info-tooltip" data-tip="How often a screenshot is captured while monitoring is active. Lower values capture more detail but use more storage.">i</span></label>
  <input id="interval" type="number" bind:value={config.screenshot_interval_secs} min="1" max="60" />
</div>

<div class="form-group">
  <label for="batch">Batch Interval (seconds) <span class="info-tooltip" data-tip="How often captured screenshots are sent to the AI for analysis. A batch groups multiple screenshots together for efficient processing.">i</span></label>
  <input id="batch" type="number" bind:value={config.batch_interval_secs} min="60" max="3600" />
</div>

<div class="form-group">
  <label for="dedup">Deduplication Threshold (Sensitivity) <span class="info-tooltip" data-tip="Controls how aggressively similar screenshots are filtered out. Lower values keep more screenshots (more sensitive to changes), higher values filter more aggressively. 0 keeps all screenshots.">i</span></label>
  <input id="dedup" type="number" bind:value={config.dedup_threshold} min="0" max="64" />
</div>

<div class="form-group">
  <label for="datadir">Data Directory <span class="info-tooltip" data-tip="The folder where screenshots, extracted data, and reports are stored. Leave empty to use the default location.">i</span></label>
  <input id="datadir" type="text" bind:value={config.data_dir} />
</div>

<hr class="section-divider" />

<h2>Startup</h2>

<div class="toggle-group">
  <span class="toggle-label">Launch Diaroo at system startup <span class="info-tooltip" data-tip="Automatically open Diaroo when you log in to your computer.">i</span></span>
  <label class="toggle-switch">
    <input type="checkbox" bind:checked={config.launch_at_startup} />
    <span class="toggle-slider"></span>
  </label>
</div>

<hr class="section-divider" />

<h2>Monitoring</h2>

<div class="toggle-group">
  <span class="toggle-label">Start monitoring at scheduled time <span class="info-tooltip" data-tip="Automatically begin capturing screenshots at the specified time each day, so you don't have to start it manually.">i</span></span>
  <label class="toggle-switch">
    <input type="checkbox" bind:checked={config.auto_start_monitoring_time_enabled} />
    <span class="toggle-slider"></span>
  </label>
</div>

{#if config.auto_start_monitoring_time_enabled}
  <div class="form-group">
    <label for="auto-start-time">Start Time <span class="info-tooltip" data-tip="The time of day when monitoring will automatically begin.">i</span></label>
    <input id="auto-start-time" type="time" bind:value={config.auto_start_monitoring_time} />
  </div>
{/if}

<hr class="section-divider" />

<h2>Auto Report</h2>

<div class="toggle-group">
  <span class="toggle-label">Generate daily report automatically <span class="info-tooltip" data-tip="Automatically create a summary report of your day's activity at the specified time.">i</span></span>
  <label class="toggle-switch">
    <input type="checkbox" bind:checked={config.auto_report_enabled} />
    <span class="toggle-slider"></span>
  </label>
</div>

{#if config.auto_report_enabled}
  <div class="form-group">
    <label for="auto-report-time">Report Time <span class="info-tooltip" data-tip="The time of day when the daily report will be automatically generated.">i</span></label>
    <input id="auto-report-time" type="time" bind:value={config.auto_report_time} />
  </div>
{/if}

<hr class="section-divider" />

<h2>Prompts</h2>

<div class="prompt-edit-group">
  <span class="toggle-label">Customize the batch extract prompt <span class="info-tooltip" data-tip="Edit the prompt sent to the AI when analyzing a batch of screenshots. Controls what information gets extracted from your screen activity.">i</span></span>
  <button class="secondary-btn" onclick={editExtractPrompt}>Edit Extract Prompt</button>
</div>

<div class="prompt-edit-group">
  <span class="toggle-label">Customize the daily report prompt <span class="info-tooltip" data-tip="Edit the prompt used to generate your daily activity summary report. Controls the format and detail level of reports.">i</span></span>
  <button class="secondary-btn" onclick={editPrompt}>Edit Report Prompt</button>
</div>

<hr class="section-divider" />

<button onclick={save}>Save Settings</button>

{#if statusMessage}
  <div class="status">{statusMessage}</div>
{/if}
