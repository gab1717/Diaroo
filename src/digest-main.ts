import { mount } from "svelte";
import DigestPicker from "./DigestPicker.svelte";

const app = mount(DigestPicker, {
  target: document.getElementById("digest-app")!,
});

export default app;
