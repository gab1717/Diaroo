import { writable } from "svelte/store";

export interface PetState {
  x: number;
  y: number;
  animation: string;
  frame: number;
  isDragging: boolean;
}

export const petState = writable<PetState>({
  x: 200,
  y: 200,
  animation: "idle",
  frame: 0,
  isDragging: false,
});
