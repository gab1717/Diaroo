import { mount } from "svelte";
import PetPicker from "./PetPicker.svelte";

const app = mount(PetPicker, {
  target: document.getElementById("pet-picker-app")!,
});

export default app;
