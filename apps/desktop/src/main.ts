import { timing } from "./app-timing";
import { mount } from "svelte";
import App from "./App.svelte";
import "./style.css";
const mounted = timing({ kind: "app_mount" }, "mount_commit");
const frame = timing({ kind: "app_mount" }, "next_frame");
try { mount(App, { target: document.getElementById("app")! }); mounted("complete"); requestAnimationFrame(() => frame("complete")); }
catch (error) { mounted("failed"); frame("abandoned"); throw error; }
