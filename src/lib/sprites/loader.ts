import { invoke } from "@tauri-apps/api/core";
import type { PetInfo } from "./types";

export interface LoadedAnimation {
  image: HTMLImageElement;
  frames: number;
  frameDuration: number;
  loop: boolean;
}

export async function loadPetAnimations(
  petInfo: PetInfo,
): Promise<Record<string, LoadedAnimation>> {
  const animations: Record<string, LoadedAnimation> = {};

  for (const [name, animDef] of Object.entries(petInfo.animations)) {
    const filePath = petInfo.spritePaths[name];
    if (!filePath) continue;

    const b64: string = await invoke("read_sprite", { path: filePath });
    const image = await loadImage(`data:image/png;base64,${b64}`);

    animations[name] = {
      image,
      frames: animDef.frames,
      frameDuration: animDef.frameDuration,
      loop: animDef.loop,
    };
  }

  return animations;
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = (_e) => reject(new Error(`Failed to load image: ${src}`));
    img.src = src;
  });
}

export function drawFrame(
  ctx: CanvasRenderingContext2D,
  anim: LoadedAnimation,
  frameIndex: number,
  spriteSize: number,
  renderSize: number,
  flipped?: boolean,
): void {
  ctx.clearRect(0, 0, renderSize, renderSize);
  if (flipped) {
    ctx.save();
    ctx.translate(renderSize, 0);
    ctx.scale(-1, 1);
  }
  ctx.drawImage(
    anim.image,
    frameIndex * spriteSize, // source x
    0,                       // source y
    spriteSize,              // source width
    spriteSize,              // source height
    0,                       // dest x
    0,                       // dest y
    renderSize,              // dest width
    renderSize,              // dest height
  );
  if (flipped) {
    ctx.restore();
  }
}
