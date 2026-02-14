<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { getConfig, setConfig, listPets, installPet, removePet, type AppConfig } from "./lib/api/commands";
  import { open } from "@tauri-apps/plugin-dialog";
  import { loadPetAnimations, drawFrame, type LoadedAnimation } from "./lib/sprites/loader";
  import type { PetInfo } from "./lib/sprites/types";

  let config = $state<AppConfig | null>(null);
  let pets = $state<PetInfo[]>([]);
  let statusMessage = $state("");
  let canvases: Record<string, HTMLCanvasElement> = {};
  let animationData: Record<string, { anim: LoadedAnimation; spriteSize: number }> = {};
  let animFrameId: number | null = null;

  async function loadData() {
    try {
      config = await getConfig();
      pets = await listPets();
    } catch (e) {
      console.error("Failed to load data:", e);
    }
  }

  onMount(async () => {
    await loadData();
    await startAnimations();
  });

  onDestroy(() => {
    if (animFrameId !== null) {
      cancelAnimationFrame(animFrameId);
    }
  });

  async function startAnimations() {
    // Load animations for each pet
    for (const pet of pets) {
      try {
        const anims = await loadPetAnimations(pet);
        const idleKey = pet.defaultAnimation || "idle";
        const anim = anims[idleKey];
        if (anim) {
          animationData[pet.name] = { anim, spriteSize: pet.spriteSize };
        }
      } catch (e) {
        console.error(`Failed to load animations for ${pet.name}:`, e);
      }
    }

    // Start animation loop
    let lastTime = 0;
    function animate(time: number) {
      if (lastTime === 0) lastTime = time;

      for (const pet of pets) {
        const data = animationData[pet.name];
        const canvas = canvases[pet.name];
        if (!data || !canvas) continue;

        const ctx = canvas.getContext("2d");
        if (!ctx) continue;

        ctx.imageSmoothingEnabled = false;
        const { anim, spriteSize } = data;
        const totalDuration = anim.frames * anim.frameDuration;
        const frameIndex = Math.floor((time / anim.frameDuration) % anim.frames);
        drawFrame(ctx, anim, frameIndex, spriteSize, canvas.width);
      }

      animFrameId = requestAnimationFrame(animate);
    }

    animFrameId = requestAnimationFrame(animate);
  }

  async function selectPet(name: string) {
    if (!config) return;
    try {
      config.pet_name = name;
      await setConfig(config);
      statusMessage = `Switched to ${name}!`;
      setTimeout(() => (statusMessage = ""), 2000);
    } catch (e) {
      statusMessage = `Error: ${e}`;
    }
  }

  async function addPet() {
    try {
      const path = await open({
        multiple: false,
        filters: [{ name: "Pet Package", extensions: ["dpet"] }],
      });
      if (!path) return;
      await installPet(path);
      statusMessage = "Pet installed!";
      setTimeout(() => (statusMessage = ""), 3000);
      // Reload and restart animations
      if (animFrameId !== null) {
        cancelAnimationFrame(animFrameId);
        animFrameId = null;
      }
      animationData = {};
      canvases = {};
      await loadData();
      // Wait a tick for DOM to update before starting animations
      await new Promise((r) => setTimeout(r, 0));
      await startAnimations();
    } catch (e) {
      statusMessage = `Error: ${e}`;
    }
  }

  async function handleRemovePet(name: string) {
    try {
      await removePet(name);
      if (config && config.pet_name === name) {
        config.pet_name = "cat";
        await setConfig(config);
      }
      statusMessage = "Pet removed!";
      setTimeout(() => (statusMessage = ""), 3000);
      // Reload
      delete animationData[name];
      delete canvases[name];
      await loadData();
    } catch (e) {
      statusMessage = `Error: ${e}`;
    }
  }

  function bindCanvas(el: HTMLCanvasElement, name: string) {
    canvases[name] = el;
  }
</script>

<h1>Pets</h1>

<div class="pet-grid">
  {#each pets as pet (pet.name)}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="pet-card"
      class:active={config?.pet_name === pet.name}
      onclick={() => selectPet(pet.name)}
      onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter' || e.key === ' ') selectPet(pet.name); }}
      role="button"
      tabindex="0"
    >
      <canvas
        use:bindCanvas={pet.name}
        width="64"
        height="64"
        class="pet-canvas"
      ></canvas>
      <div class="pet-label">
        <strong>{pet.displayName}</strong>
        {#if pet.version}
          <span class="pet-version">v{pet.version}</span>
        {/if}
      </div>
      <button
        class="remove-btn"
        onclick={(e: MouseEvent) => { e.stopPropagation(); handleRemovePet(pet.name); }}
      >Remove</button>
    </div>
  {/each}
</div>

<button class="add-pet-btn" onclick={addPet}>Add Pet (.dpet)</button>

{#if statusMessage}
  <div class="status">{statusMessage}</div>
{/if}

<style>
  .pet-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
    gap: 12px;
    margin-bottom: 16px;
  }

  .pet-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 12px 8px;
    background: #16213e;
    border: 2px solid #333;
    border-radius: 10px;
    cursor: pointer;
    transition: border-color 0.15s, background 0.15s;
    color: #eee;
    font-size: inherit;
    font-weight: normal;
  }

  .pet-card:hover {
    border-color: #f4a035;
    background: #1e2a4a;
  }

  .pet-card.active {
    border-color: #f4a035;
    background: #2a1a0e;
    box-shadow: 0 0 12px rgba(244, 160, 53, 0.3);
  }

  .pet-canvas {
    image-rendering: pixelated;
    width: 64px;
    height: 64px;
  }

  .pet-label {
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }

  .pet-version {
    color: #888;
    font-size: 0.75rem;
  }

  .remove-btn {
    background: #fee2e2;
    color: #dc2626;
    border: none;
    padding: 0.2rem 0.5rem;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.75rem;
  }

  .remove-btn:hover {
    background: #fecaca;
  }

  .add-pet-btn {
    width: 100%;
    padding: 0.5rem;
    background: #0d1b2a;
    color: #4ade80;
    border: 1px dashed #4ade80;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.9rem;
  }

  .add-pet-btn:hover {
    background: #162032;
  }
</style>
