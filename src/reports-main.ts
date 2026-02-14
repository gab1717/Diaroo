import { mount } from "svelte";
import Reports from "./Reports.svelte";

const app = mount(Reports, {
  target: document.getElementById("reports-app")!,
});

export default app;
