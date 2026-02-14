<script lang="ts">
  import { onMount } from "svelte";
  import {
    listDataDates,
    generateDigest,
    type DateInfo,
  } from "./lib/api/commands";
  import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { emitTo } from "@tauri-apps/api/event";

  let dates: DateInfo[] = $state([]);
  let selectedDate: string | null = $state(null);
  let loading: boolean = $state(true);
  let status: "idle" | "generating" | "success" | "no-data" | "error" =
    $state("idle");
  let errorMessage: string = $state("");
  let isGenerating: boolean = $state(false);
  let statusDate: string = $state("");
  let selectedHasReport: boolean = $derived(
    dates.some((d) => d.date === selectedDate && d.has_report),
  );

  interface MonthGroup {
    label: string;
    days: { date: string; display: string; has_report: boolean }[];
  }

  let grouped: MonthGroup[] = $derived.by(() => {
    const map = new Map<
      string,
      { date: string; display: string; has_report: boolean }[]
    >();
    for (const info of dates) {
      const d = new Date(info.date + "T00:00:00");
      const monthKey = d.toLocaleDateString("en-US", {
        year: "numeric",
        month: "long",
      });
      const dayDisplay =
        d.getDate() +
        " " +
        d.toLocaleDateString("en-US", { weekday: "short" });
      if (!map.has(monthKey)) map.set(monthKey, []);
      map.get(monthKey)!.push({
        date: info.date,
        display: dayDisplay,
        has_report: info.has_report,
      });
    }
    const groups: MonthGroup[] = [];
    for (const [label, days] of map) {
      groups.push({ label, days });
    }
    return groups;
  });

  async function generate() {
    if (!selectedDate || isGenerating) return;
    isGenerating = true;
    status = "generating";
    statusDate = selectedDate;
    errorMessage = "";
    try {
      await generateDigest(selectedDate);
      status = "success";
      // Refresh the list to update has_report badges
      dates = await listDataDates();
    } catch (e: any) {
      const msg = typeof e === "string" ? e : e?.message ?? String(e);
      if (
        msg.toLowerCase().includes("no activity") ||
        msg.toLowerCase().includes("no data") ||
        msg.toLowerCase().includes("no entries")
      ) {
        status = "no-data";
      } else {
        status = "error";
        errorMessage = msg;
      }
    } finally {
      isGenerating = false;
    }
  }

  async function viewReport(date: string) {
    const existing = await WebviewWindow.getByLabel("reports");
    if (existing) {
      await existing.show();
      await existing.setFocus();
      await emitTo("reports", "select-report-date", date);
    } else {
      const win = new WebviewWindow("reports", {
        url: "reports.html",
        title: "Diaroo - Reports",
        width: 800,
        height: 600,
      });
      win.once("tauri://created", async () => {
        // Small delay to let the Svelte app mount and set up its listener
        setTimeout(() => emitTo("reports", "select-report-date", date), 300);
      });
    }
  }

  onMount(async () => {
    dates = await listDataDates();
    // Default to today if it exists in the list
    const today = new Date().toISOString().slice(0, 10);
    const todayEntry = dates.find((d) => d.date === today);
    if (todayEntry) {
      selectedDate = today;
    } else if (dates.length > 0) {
      selectedDate = dates[0].date;
    }
    loading = false;
  });
</script>

<div class="container">
  <h1>Generate Digest</h1>

  {#if loading}
    <p class="empty-state">Loading...</p>
  {:else if dates.length === 0}
    <p class="empty-state">No activity data found</p>
  {:else}
    <div class="date-list">
      {#each grouped as group}
        <div class="month-group">
          <div class="month-header">{group.label}</div>
          {#each group.days as day}
            <button
              class="day-entry"
              class:active={selectedDate === day.date}
              onclick={() => {
                selectedDate = day.date;
              }}
            >
              <span>{day.display}</span>
              {#if day.has_report}
                <span class="badge" role="button" tabindex="-1" onclick={(e) => { e.stopPropagation(); viewReport(day.date); }} onkeydown={(e) => { if (e.key === 'Enter') { e.stopPropagation(); viewReport(day.date); } }}>
                  view report
                </span>
              {/if}
            </button>
          {/each}
        </div>
      {/each}
    </div>

    <div class="actions">
      <div class="status" class:generating={status === "generating"} class:success={status === "success"} class:warn={status === "no-data"} class:error={status === "error"}>
        {#if status === "generating"}
          Generating report for {statusDate}...
        {:else if status === "success"}
          Report generated for {statusDate}.
        {:else if status === "no-data"}
          No activity data found for {statusDate}.
        {:else if status === "error"}
          Error: {errorMessage}
        {:else}
          &nbsp;
        {/if}
      </div>

      <button
        class="generate-btn"
        onclick={generate}
        disabled={!selectedDate || isGenerating}
      >
        {isGenerating ? "Generating..." : selectedHasReport ? "Regenerate Report" : "Generate Report"}
      </button>
    </div>
  {/if}
</div>

<style>
  .container {
    display: flex;
    flex-direction: column;
    height: 100vh;
    padding: 20px;
  }

  h1 {
    font-size: 1.2rem;
    color: #f4a035;
    margin-bottom: 16px;
  }

  .empty-state {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #888;
  }

  .date-list {
    flex: 1;
    overflow-y: auto;
    border: 1px solid #333;
    border-radius: 6px;
    background: #16213e;
  }

  .month-group {
    margin-bottom: 4px;
  }

  .month-header {
    font-size: 0.75rem;
    color: #888;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 10px 16px 4px;
    position: sticky;
    top: 0;
    background: #16213e;
  }

  .day-entry {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    text-align: left;
    padding: 6px 16px 6px 24px;
    background: none;
    border: none;
    color: #ccc;
    font-size: 0.85rem;
    cursor: pointer;
    font-weight: 400;
  }

  .day-entry:hover {
    background: rgba(244, 160, 53, 0.1);
    color: #fff;
  }

  .day-entry.active {
    background: rgba(244, 160, 53, 0.2);
    color: #f4a035;
    font-weight: 600;
  }

  .badge {
    font-size: 0.65rem;
    color: #4cd964;
    background: none;
    border: 1px solid #4cd964;
    border-radius: 3px;
    padding: 1px 5px;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    cursor: pointer;
  }

  .badge:hover {
    background: rgba(76, 217, 100, 0.15);
    color: #fff;
  }

  .actions {
    padding-top: 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .status {
    font-size: 0.85rem;
    padding: 8px 12px;
    border-radius: 4px;
  }

  .status.generating {
    color: #aaa;
    background: rgba(255, 255, 255, 0.05);
  }

  .status.success {
    color: #4cd964;
    background: rgba(76, 217, 100, 0.1);
  }

  .status.warn {
    color: #f4a035;
    background: rgba(244, 160, 53, 0.1);
  }

  .status.error {
    color: #ff6b6b;
    background: rgba(255, 107, 107, 0.1);
  }

  .generate-btn {
    padding: 10px 20px;
    font-size: 0.95rem;
    font-weight: 600;
    background: #f4a035;
    color: #1a1a2e;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    transition: opacity 0.15s;
  }

  .generate-btn:hover:not(:disabled) {
    opacity: 0.9;
  }

  .generate-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
