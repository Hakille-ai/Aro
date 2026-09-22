import App from "./App.svelte";
import "./styles.css";
import "./styles/app/01-previews-spotlight-shell.css";
import "./styles/app/02-voice-core.css";
import "./styles/app/03-settings-dashboard-organization.css";
import "./styles/app/04-modals-markdown-chat.css";
import "./styles/app/05-memory-settings-arena.css";
import "./styles/app/06-splash-auth-command.css";
import "./styles/app/07-responsive-agent.css";
import "./styles/app/08-web-saas.css";
import "./lib/chatActions";
import { mount } from "svelte";

const app = mount(App, {
  target: document.getElementById("app") as HTMLElement,
});

export default app;
// Reload trigger: 2026-09-04 04:21:40
