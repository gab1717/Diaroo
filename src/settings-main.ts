import { mount } from "svelte";
import Settings from "./Settings.svelte";

const app = mount(Settings, {
  target: document.getElementById("settings-app")!,
});

export default app;
