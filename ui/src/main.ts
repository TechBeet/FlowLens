import { mount } from "svelte";
import App from "./lib/App.svelte";
import "./styles/global.css";

const target = document.getElementById("root");
if (!target) {
  throw new Error("Root element #root not found");
}

const app = mount(App, { target });

export default app;