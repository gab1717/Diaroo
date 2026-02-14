<script lang="ts">
  import { onMount } from "svelte";
  import { listReports, readReport, openReportFile } from "./lib/api/commands";
  import { listen } from "@tauri-apps/api/event";
  import { marked } from "marked";

  let dates: string[] = $state([]);
  let selectedDate: string | null = $state(null);
  let reportHtml: string = $state("");
  let loading: boolean = $state(true);

  interface MonthGroup {
    label: string;
    days: { date: string; display: string }[];
  }

  let grouped: MonthGroup[] = $derived.by(() => {
    const map = new Map<string, { date: string; display: string }[]>();
    for (const date of dates) {
      const d = new Date(date + "T00:00:00");
      const monthKey = d.toLocaleDateString("en-US", {
        year: "numeric",
        month: "long",
      });
      const dayDisplay =
        d.getDate() +
        " " +
        d.toLocaleDateString("en-US", { weekday: "short" });
      if (!map.has(monthKey)) map.set(monthKey, []);
      map.get(monthKey)!.push({ date, display: dayDisplay });
    }
    const groups: MonthGroup[] = [];
    for (const [label, days] of map) {
      groups.push({ label, days });
    }
    return groups;
  });

  async function selectDate(date: string) {
    selectedDate = date;
    const md = await readReport(date);
    reportHtml = await marked.parse(md);
  }

  onMount(async () => {
    dates = await listReports();
    if (dates.length > 0) {
      await selectDate(dates[0]);
    }
    loading = false;

    listen<string>("select-report-date", async (event) => {
      // Refresh the list in case a new report was just generated
      dates = await listReports();
      if (dates.includes(event.payload)) {
        await selectDate(event.payload);
      }
    });
  });
</script>

<div class="layout">
  <aside class="sidebar">
    <h2>Reports</h2>
    {#if loading}
      <p class="sidebar-empty">Loading...</p>
    {:else if dates.length === 0}
      <p class="sidebar-empty">No reports yet</p>
    {:else}
      {#each grouped as group}
        <div class="month-group">
          <div class="month-header">{group.label}</div>
          {#each group.days as day}
            <button
              class="day-entry"
              class:active={selectedDate === day.date}
              onclick={() => selectDate(day.date)}
            >
              {day.display}
            </button>
          {/each}
        </div>
      {/each}
    {/if}
  </aside>

  <main class="viewer">
    {#if loading}
      <p class="empty-state">Loading...</p>
    {:else if dates.length === 0}
      <p class="empty-state">No reports yet</p>
    {:else if selectedDate}
      <div class="report-header">
        <span>{selectedDate}</span>
        <button class="open-file-btn" onclick={() => openReportFile(selectedDate!)}>
          Open File
        </button>
      </div>
      <div class="report-content">
        {@html reportHtml}
      </div>
    {/if}
  </main>
</div>

<style>
  .layout {
    display: flex;
    height: 100vh;
  }

  .sidebar {
    width: 220px;
    min-width: 220px;
    background: #16213e;
    border-right: 1px solid #333;
    overflow-y: auto;
    padding: 16px 0;
  }

  .sidebar h2 {
    font-size: 1.1rem;
    color: #f4a035;
    padding: 0 16px 12px;
    border-bottom: 1px solid #333;
    margin-bottom: 8px;
  }

  .sidebar-empty {
    padding: 16px;
    color: #888;
    font-size: 0.85rem;
  }

  .month-group {
    margin-bottom: 8px;
  }

  .month-header {
    font-size: 0.75rem;
    color: #888;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 8px 16px 4px;
  }

  .day-entry {
    display: block;
    width: 100%;
    text-align: left;
    padding: 6px 16px 6px 24px;
    background: none;
    border: none;
    color: #ccc;
    font-size: 0.85rem;
    cursor: pointer;
    border-radius: 0;
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

  .viewer {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .report-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 24px;
    font-size: 1rem;
    font-weight: 600;
    color: #f4a035;
    border-bottom: 1px solid #333;
  }

  .open-file-btn {
    padding: 4px 12px;
    font-size: 0.8rem;
    background: transparent;
    color: #aaa;
    border: 1px solid #555;
    border-radius: 4px;
    cursor: pointer;
    font-weight: 400;
  }

  .open-file-btn:hover {
    color: #eee;
    border-color: #888;
    background: rgba(255, 255, 255, 0.05);
  }

  .report-content {
    flex: 1;
    overflow-y: auto;
    padding: 24px;
    line-height: 1.6;
  }

  .report-content :global(h1) {
    font-size: 1.4rem;
    margin-bottom: 12px;
    color: #f4a035;
  }

  .report-content :global(h2) {
    font-size: 1.15rem;
    margin: 20px 0 8px;
    color: #ddd;
  }

  .report-content :global(h3) {
    font-size: 1rem;
    margin: 16px 0 6px;
    color: #ccc;
  }

  .report-content :global(p) {
    margin-bottom: 10px;
  }

  .report-content :global(ul),
  .report-content :global(ol) {
    margin: 8px 0 8px 24px;
  }

  .report-content :global(li) {
    margin-bottom: 4px;
  }

  .report-content :global(code) {
    background: #16213e;
    padding: 2px 6px;
    border-radius: 3px;
    font-size: 0.85em;
  }

  .report-content :global(pre) {
    background: #16213e;
    padding: 12px;
    border-radius: 6px;
    overflow-x: auto;
    margin: 10px 0;
  }

  .report-content :global(pre code) {
    padding: 0;
    background: none;
  }

  .report-content :global(blockquote) {
    border-left: 3px solid #f4a035;
    padding-left: 12px;
    color: #aaa;
    margin: 10px 0;
  }

  .report-content :global(a) {
    color: #f4a035;
  }

  .report-content :global(hr) {
    border: none;
    border-top: 1px solid #333;
    margin: 16px 0;
  }

  .empty-state {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #888;
    font-size: 1rem;
  }
</style>
