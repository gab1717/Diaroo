<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { getCurrentWindow, currentMonitor } from "@tauri-apps/api/window";
  import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
  import { loadPetAnimations, drawFrame, type LoadedAnimation } from "../sprites/loader";
  import { getConfig, getPetInfo, savePetPosition } from "../api/commands";
  import { onPetSizeChanged, onPetChanged, onActivityTick, onWanderToggled } from "../api/events";
  import { BehaviorEngine } from "../behavior/engine";
  import { ActivitySignalAccumulator } from "../behavior/signals";

  function sizeNameToRenderSize(name: string): number {
    switch (name) {
      case "small": return 64;
      case "large": return 144;
      default: return 96; // medium
    }
  }

  let spriteSize = $state(16);
  let renderSize = $state(64);

  let canvas: HTMLCanvasElement;
  let ctx: CanvasRenderingContext2D | null = null;
  let animations: Record<string, LoadedAnimation> = {};
  let currentAnimation = "idle";
  let currentFrame = 0;
  let frameElapsed = 0;
  let flipped = false;
  let unlistenSize: (() => void) | null = null;
  let unlistenPet: (() => void) | null = null;
  let unlistenActivity: (() => void) | null = null;
  let unlistenWander: (() => void) | null = null;
  const signals = new ActivitySignalAccumulator();
  let loadError = $state(false);
  let rafId: number | null = null;
  let lastTime = 0;

  let isDragging = $state(false);
  let isBouncing = $state(false);
  let positionSaveInterval: ReturnType<typeof setInterval> | null = null;

  // Click vs drag detection
  let pointerStart: { x: number; y: number } | null = null;
  let pointerIsDown = false;
  const DRAG_THRESHOLD = 3;

  // Particle system
  interface Particle {
    x: number;
    y: number;
    vx: number;
    vy: number;
    opacity: number;
    color: string;
    size: number;
  }
  let particleCanvas: HTMLCanvasElement;
  let particleCtx: CanvasRenderingContext2D | null = null;
  let particles: Particle[] = [];

  const HEART_PATTERN = [
    [0,1,0,1,0],
    [1,1,1,1,1],
    [1,1,1,1,1],
    [0,1,1,1,0],
    [0,0,1,0,0],
  ];
  const HEART_COLORS = ["#ff6b9d", "#ff4081", "#e91e63", "#f06292", "#ff80ab"];

  const appWindow = getCurrentWindow();

  // Behavior engine
  let engine: BehaviorEngine | null = null;
  let positionUpdateFrame = 0;
  let displayScaleFactor = 1;

  function initEngine(screenW: number, screenH: number, petSz: number, startX: number, startY: number) {
    engine = new BehaviorEngine(
      {
        onAnimationChange: (anim, dir) => {
          if (currentAnimation !== anim) {
            currentAnimation = anim;
            currentFrame = 0;
            frameElapsed = 0;
          }
          flipped = dir === 1;
        },
        onPositionChange: (x, y) => {
          // Throttle position IPC to every other frame (~30fps)
          positionUpdateFrame++;
          if (positionUpdateFrame % 2 === 0) {
            appWindow.setPosition(new LogicalPosition(Math.round(x), Math.round(y)));
          }
        },
        onStateChange: (state) => {
          // When entering a movement state, read actual window position first.
          // This prevents snap-back after drag.
          if (engine?.needsPositionSync) {
            appWindow.outerPosition().then((pos) => {
              const sf = displayScaleFactor;
              engine?.syncPosition(pos.x / sf, pos.y / sf);
            }).catch(() => {
              // Can't read position — unblock movement at current engine coords
              engine?.syncPosition(engine.position.x, engine.position.y);
            });
          }
        },
      },
      screenW,
      screenH,
      petSz,
      startX,
      startY,
      Object.keys(animations),
    );
  }

  // --- Game Loop ---

  function gameLoop(now: number) {
    if (!lastTime) lastTime = now;
    const dt = Math.min((now - lastTime) / 1000, 0.1); // cap to avoid spiral
    lastTime = now;

    // Update behavior engine
    if (engine && !isDragging) {
      engine.update(now, dt);
    }

    // Advance sprite frame
    const anim = animations[currentAnimation];
    if (anim && ctx) {
      frameElapsed += dt * 1000;
      if (frameElapsed >= anim.frameDuration) {
        frameElapsed -= anim.frameDuration;
        currentFrame = (currentFrame + 1) % anim.frames;
      }
      drawFrame(ctx, anim, currentFrame, spriteSize, renderSize, flipped);
    }

    // Particles
    if (particles.length > 0) {
      updateParticles();
    }

    rafId = requestAnimationFrame(gameLoop);
  }

  function redrawCurrentFrame() {
    const anim = animations[currentAnimation];
    if (anim && ctx) {
      drawFrame(ctx, anim, currentFrame, spriteSize, renderSize, flipped);
    }
  }

  async function loadPet(petName: string) {
    loadError = false;
    try {
      const petInfo = await getPetInfo(petName);
      spriteSize = petInfo.spriteSize;
      currentAnimation = petInfo.defaultAnimation;
      currentFrame = 0;
      frameElapsed = 0;
      animations = await loadPetAnimations(petInfo);

      await appWindow.setSize(new LogicalSize(renderSize, renderSize));

      if (engine) {
        engine.setAvailableAnimations(Object.keys(animations));
        const pos = engine.position;
        engine.reset(pos.x, pos.y);
      }

      queueMicrotask(() => {
        ctx = canvas.getContext("2d");
        if (ctx) ctx.imageSmoothingEnabled = false;
        redrawCurrentFrame();
      });
    } catch (e) {
      console.error("Failed to load pet:", e);
      loadError = true;
    }
  }

  // --- Click vs Drag Detection ---

  function onPointerDown(e: PointerEvent) {
    pointerIsDown = true;
    pointerStart = { x: e.clientX, y: e.clientY };
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
  }

  async function onPointerMove(e: PointerEvent) {
    if (!pointerIsDown || !pointerStart) return;
    const dx = e.clientX - pointerStart.x;
    const dy = e.clientY - pointerStart.y;
    if (dx * dx + dy * dy > DRAG_THRESHOLD * DRAG_THRESHOLD) {
      pointerIsDown = false;
      pointerStart = null;
      isDragging = true;
      await appWindow.startDragging();
      // startDragging() resolves before the OS drag ends on Windows.
      // Keep isDragging true and poll until the window position stabilizes.
      const finalPos = await waitForDragEnd();
      isDragging = false;
      engine?.syncPosition(finalPos.x, finalPos.y);
      savePetPosition(finalPos.x, finalPos.y).catch(() => {});
    }
  }

  function onPointerUp(e: PointerEvent) {
    if (pointerIsDown && pointerStart) {
      triggerClickReaction(e);
    }
    pointerIsDown = false;
    pointerStart = null;
  }

  /** Poll outerPosition until stable — detects when the OS drag actually ends. */
  async function waitForDragEnd(): Promise<{ x: number; y: number }> {
    const sf = displayScaleFactor;
    let lastX = NaN;
    let lastY = NaN;
    let stableCount = 0;

    while (stableCount < 2) {
      await new Promise((r) => setTimeout(r, 120));
      try {
        const pos = await appWindow.outerPosition();
        const x = pos.x / sf;
        const y = pos.y / sf;
        if (!isNaN(lastX) && Math.abs(x - lastX) < 1 && Math.abs(y - lastY) < 1) {
          stableCount++;
        } else {
          stableCount = 0;
        }
        lastX = x;
        lastY = y;
      } catch {
        break;
      }
    }
    return {
      x: isNaN(lastX) ? engine?.position.x ?? 0 : lastX,
      y: isNaN(lastY) ? engine?.position.y ?? 0 : lastY,
    };
  }

  // --- Click Reaction ---

  function triggerClickReaction(e: PointerEvent) {
    // Bounce animation
    isBouncing = false;
    requestAnimationFrame(() => {
      isBouncing = true;
    });
    spawnParticles(e);

  }

  function onBounceEnd() {
    isBouncing = false;
  }

  // --- Particle System ---

  function drawHeart(pCtx: CanvasRenderingContext2D, x: number, y: number, pixelSize: number, color: string, opacity: number) {
    pCtx.fillStyle = color;
    pCtx.globalAlpha = opacity;
    const offset = 2.5 * pixelSize;
    for (let row = 0; row < HEART_PATTERN.length; row++) {
      for (let col = 0; col < HEART_PATTERN[row].length; col++) {
        if (HEART_PATTERN[row][col]) {
          pCtx.fillRect(
            Math.round(x + col * pixelSize - offset),
            Math.round(y + row * pixelSize - offset),
            pixelSize,
            pixelSize
          );
        }
      }
    }
    pCtx.globalAlpha = 1;
  }

  function spawnParticles(e: PointerEvent) {
    const rect = particleCanvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const pxSize = Math.max(1, Math.floor(renderSize / spriteSize / 2));

    for (let i = 0; i < 6; i++) {
      particles.push({
        x: x + (Math.random() - 0.5) * 10,
        y: y + (Math.random() - 0.5) * 10,
        vx: (Math.random() - 0.5) * 1.5,
        vy: -(Math.random() * 1.5 + 0.5),
        opacity: 1,
        color: HEART_COLORS[Math.floor(Math.random() * HEART_COLORS.length)],
        size: pxSize,
      });
    }
  }

  function updateParticles() {
    if (!particleCtx) {
      particleCtx = particleCanvas.getContext("2d");
    }
    if (!particleCtx) return;

    particleCtx.clearRect(0, 0, particleCanvas.width, particleCanvas.height);

    for (const p of particles) {
      p.x += p.vx;
      p.y += p.vy;
      p.opacity -= 0.02;
    }
    particles = particles.filter(p => p.opacity > 0.01);

    for (const p of particles) {
      drawHeart(particleCtx, p.x, p.y, p.size, p.color, p.opacity);
    }
  }

  onMount(async () => {
    let petName = "cat";
    let wanderEnabled = true;
    try {
      const config = await getConfig();
      renderSize = sizeNameToRenderSize(config.pet_size);
      petName = config.pet_name;
      wanderEnabled = config.wander_enabled ?? true;
    } catch {
      // keep defaults
    }

    ctx = canvas.getContext("2d");
    if (ctx) ctx.imageSmoothingEnabled = false;

    await loadPet(petName);

    // Get screen bounds and initial position for movement (all in logical pixels)
    try {
      const monitor = await currentMonitor();
      const pos = await appWindow.outerPosition();
      if (monitor) {
        const sf = monitor.scaleFactor;
        displayScaleFactor = sf;
        const screenW = monitor.size.width / sf;
        const screenH = monitor.size.height / sf;
        initEngine(screenW, screenH, renderSize, pos.x / sf, pos.y / sf);
      }
    } catch (e) {
      console.warn("Could not get monitor info, using fallback bounds:", e);
      initEngine(1920, 1080, renderSize, 200, 800);
    }

    engine?.setWanderEnabled(wanderEnabled);

    // Periodically save pet position (every 30s)
    positionSaveInterval = setInterval(async () => {
      try {
        const pos = await appWindow.outerPosition();
        const sf = displayScaleFactor;
        savePetPosition(pos.x / sf, pos.y / sf).catch(() => {});
      } catch {
        // ignore
      }
    }, 30_000);

    // Start the game loop
    lastTime = 0;
    rafId = requestAnimationFrame(gameLoop);

    const unlisten = await onPetSizeChanged(async (newRenderSize) => {
      renderSize = newRenderSize;
      await appWindow.setSize(new LogicalSize(renderSize, renderSize));
      try {
        const monitor = await currentMonitor();
        if (monitor && engine) {
          const sf = monitor.scaleFactor;
          displayScaleFactor = sf;
          engine.updateBounds(monitor.size.width / sf, monitor.size.height / sf, renderSize);
        }
      } catch {
        // screen bounds unchanged
      }
      queueMicrotask(() => {
        ctx = canvas.getContext("2d");
        if (ctx) ctx.imageSmoothingEnabled = false;
        redrawCurrentFrame();
      });
    });
    unlistenSize = unlisten;

    const unlistenPetEvt = await onPetChanged((newPetName) => {
      loadPet(newPetName);
    });
    unlistenPet = unlistenPetEvt;

    const unlistenActivityEvt = await onActivityTick((tick) => {
      const payload = { ...tick, timestamp: Date.now() };
      signals.push(payload);
      engine?.setActivitySignals(
        signals.activityLevel,
        signals.switchRate,
        signals.isActive,
      );
    });
    unlistenActivity = unlistenActivityEvt;

    const unlistenWanderEvt = await onWanderToggled((enabled) => {
      engine?.setWanderEnabled(enabled);
    });
    unlistenWander = unlistenWanderEvt;
  });

  onDestroy(() => {
    if (rafId !== null) cancelAnimationFrame(rafId);
    if (positionSaveInterval !== null) clearInterval(positionSaveInterval);
    if (unlistenSize) unlistenSize();
    if (unlistenPet) unlistenPet();
    if (unlistenActivity) unlistenActivity();
    if (unlistenWander) unlistenWander();
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="pet-container"
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
>
  {#if loadError}
    <div class="error">!</div>
  {/if}
  <div
    class="canvas-wrapper"
    class:bounce={isBouncing}
    onanimationend={onBounceEnd}
  >
    <canvas
      bind:this={canvas}
      width={renderSize}
      height={renderSize}
      class="pet-canvas"
      class:dragging={isDragging}
    ></canvas>
    <canvas
      bind:this={particleCanvas}
      width={renderSize}
      height={renderSize}
      class="particle-canvas"
    ></canvas>
  </div>
</div>

<style>
  .pet-container {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: grab;
    position: relative;
  }

  .pet-container:active,
  .dragging {
    cursor: grabbing;
  }

  .canvas-wrapper {
    position: relative;
  }

  .pet-canvas {
    image-rendering: pixelated;
    display: block;
  }

  .particle-canvas {
    position: absolute;
    top: 0;
    left: 0;
    pointer-events: none;
    image-rendering: pixelated;
  }

  .bounce {
    animation: bounce 300ms ease-out;
  }

  @keyframes bounce {
    0% { transform: translateY(0); }
    40% { transform: translateY(-6px); }
    100% { transform: translateY(0); }
  }

  .error {
    position: absolute;
    top: 2px;
    right: 2px;
    background: #e74c3c;
    color: white;
    border-radius: 50%;
    width: 14px;
    height: 14px;
    font-size: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: bold;
  }
</style>
