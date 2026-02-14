// Generates a test .tpet package (ghost pet — recolored cat)
// Run: node scripts/generate-test-tpet.mjs

import { createCanvas } from "canvas";
import { writeFileSync, mkdirSync, readFileSync } from "fs";
import { execSync } from "child_process";
import { dirname, join } from "path";

// Ghost palette — whites, light blues, transparent body
const PALETTE = {
  0: [0, 0, 0, 0],            // transparent
  1: [0x6c, 0x7a, 0x89, 255], // outline — grey-blue
  2: [0xe8, 0xea, 0xed, 255], // body — pale white
  3: [0xf8, 0xf9, 0xfa, 255], // belly — bright white
  4: [0xb8, 0xc4, 0xd0, 255], // inner ears / nose — light blue-grey
  5: [0xd1, 0xd8, 0xe0, 255], // stripes — medium grey
  6: [0x2d, 0x34, 0x36, 255], // eye bg — dark
  7: [0xff, 0xff, 0xff, 255], // pupils — white (inverted ghost eyes)
};

// Reuse the same frame shapes as the cat
const IDLE_0 = [
  [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
  [0,0,1,1,0,0,0,0,0,0,1,1,0,0,0,0],
  [0,1,4,2,1,0,0,0,0,1,2,4,1,0,0,0],
  [0,1,2,2,2,1,1,1,1,2,2,2,1,0,0,0],
  [0,0,1,2,5,2,2,2,2,5,2,1,0,0,0,0],
  [0,0,1,6,7,1,2,2,1,6,7,1,0,0,0,0],
  [0,0,1,2,2,2,4,4,2,2,2,1,0,0,0,0],
  [0,0,0,1,2,3,3,3,3,2,1,0,0,0,0,0],
  [0,0,0,1,2,2,2,2,2,2,1,0,0,0,0,0],
  [0,0,1,5,2,2,2,2,2,2,5,1,0,0,0,0],
  [0,0,1,2,2,2,3,3,2,2,2,1,0,0,0,0],
  [0,0,1,2,2,2,3,3,2,2,2,1,1,0,0,0],
  [0,0,0,1,2,2,2,2,2,2,1,0,1,1,0,0],
  [0,0,0,1,2,1,0,0,1,2,1,0,0,1,0,0],
  [0,0,1,1,1,1,0,0,1,1,1,1,0,0,0,0],
  [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
];

const IDLE_1 = [
  [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
  [0,0,1,1,0,0,0,0,0,0,1,1,0,0,0,0],
  [0,1,4,2,1,0,0,0,0,1,2,4,1,0,0,0],
  [0,1,2,2,2,1,1,1,1,2,2,2,1,0,0,0],
  [0,0,1,2,5,2,2,2,2,5,2,1,0,0,0,0],
  [0,0,1,2,1,1,2,2,1,1,2,1,0,0,0,0],
  [0,0,1,2,2,2,4,4,2,2,2,1,0,0,0,0],
  [0,0,0,1,2,3,3,3,3,2,1,0,0,0,0,0],
  [0,0,0,1,2,2,2,2,2,2,1,0,0,0,0,0],
  [0,0,1,5,2,2,2,2,2,2,5,1,0,0,0,0],
  [0,0,1,2,2,2,3,3,2,2,2,1,0,0,0,0],
  [0,0,1,2,2,2,3,3,2,2,2,1,1,0,0,0],
  [0,0,0,1,2,2,2,2,2,2,1,0,1,1,0,0],
  [0,0,0,1,2,1,0,0,1,2,1,0,0,1,0,0],
  [0,0,1,1,1,1,0,0,1,1,1,1,0,0,0,0],
  [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
];

const IDLE_2 = [
  [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
  [0,0,1,1,0,0,0,0,0,0,1,1,0,0,0,0],
  [0,1,4,2,1,0,0,0,0,1,2,4,1,0,0,0],
  [0,1,2,2,2,1,1,1,1,2,2,2,1,0,0,0],
  [0,0,1,2,5,2,2,2,2,5,2,1,0,0,0,0],
  [0,0,1,6,7,1,2,2,1,6,7,1,0,0,0,0],
  [0,0,1,2,2,2,4,4,2,2,2,1,0,0,0,0],
  [0,0,0,1,2,3,3,3,3,2,1,0,0,0,0,0],
  [0,0,0,1,2,2,2,2,2,2,1,0,0,0,0,0],
  [0,0,1,5,2,2,2,2,2,2,5,1,0,0,0,0],
  [0,0,1,2,2,2,3,3,2,2,2,1,0,1,0,0],
  [0,0,1,2,2,2,3,3,2,2,2,1,1,0,0,0],
  [0,0,0,1,2,2,2,2,2,2,1,0,0,0,0,0],
  [0,0,0,1,2,1,0,0,1,2,1,0,0,0,0,0],
  [0,0,1,1,1,1,0,0,1,1,1,1,0,0,0,0],
  [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
];

const IDLE_3 = [
  [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
  [0,0,0,1,1,0,0,0,0,0,1,1,0,0,0,0],
  [0,0,1,4,2,1,0,0,0,1,2,4,1,0,0,0],
  [0,0,1,2,2,2,1,1,1,2,2,2,1,0,0,0],
  [0,0,0,1,2,5,2,2,5,2,2,1,0,0,0,0],
  [0,0,0,1,6,7,1,2,1,6,7,1,0,0,0,0],
  [0,0,0,1,2,2,4,4,2,2,2,1,0,0,0,0],
  [0,0,0,0,1,3,3,3,3,2,1,0,0,0,0,0],
  [0,0,0,1,2,2,2,2,2,2,1,0,0,0,0,0],
  [0,0,1,5,2,2,2,2,2,2,5,1,0,0,0,0],
  [0,0,1,2,2,2,3,3,2,2,2,1,0,0,0,0],
  [0,1,1,2,2,2,3,3,2,2,2,1,0,0,0,0],
  [1,1,0,1,2,2,2,2,2,2,1,0,0,0,0,0],
  [0,0,0,1,2,1,0,0,1,2,1,0,0,0,0,0],
  [0,0,1,1,1,1,0,0,1,1,1,1,0,0,0,0],
  [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
];

const FRAMES = [IDLE_0, IDLE_1, IDLE_2, IDLE_3];
const SPRITE_SIZE = 16;

// Generate ghost idle.png
const canvas = createCanvas(SPRITE_SIZE * FRAMES.length, SPRITE_SIZE);
const ctx = canvas.getContext("2d");

for (let f = 0; f < FRAMES.length; f++) {
  const frame = FRAMES[f];
  const offsetX = f * SPRITE_SIZE;
  for (let y = 0; y < SPRITE_SIZE; y++) {
    for (let x = 0; x < SPRITE_SIZE; x++) {
      const colorIndex = frame[y][x];
      const [r, g, b, a] = PALETTE[colorIndex];
      if (a === 0) continue;
      ctx.fillStyle = `rgba(${r},${g},${b},${a / 255})`;
      ctx.fillRect(offsetX + x, y, 1, 1);
    }
  }
}

// Write temp files
const tmpDir = "test-pets/ghost";
mkdirSync(join(tmpDir, "sprites"), { recursive: true });
writeFileSync(join(tmpDir, "sprites", "idle.png"), canvas.toBuffer("image/png"));
writeFileSync(join(tmpDir, "pet.json"), JSON.stringify({
  name: "ghost",
  displayName: "Spooky Ghost",
  version: "1.0.0",
  author: "Test",
  spriteSize: 16,
  animations: {
    idle: { frames: 4, frameDuration: 350, loop: true }
  },
  defaultAnimation: "idle"
}, null, 2));

console.log(`Ghost pet files written to ${tmpDir}/`);
console.log("Now zip them into a .tpet file...");
