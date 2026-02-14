import { mount } from "svelte";
import App from "./App.svelte";

document.addEventListener("contextmenu", (e) => e.preventDefault());

const app = mount(App, {
  target: document.getElementById("app")!,
});

export default app;
