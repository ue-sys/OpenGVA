import { render } from "solid-js/web";
import { App } from "./App";
import "./hmi.css";

// Bezel buttons are keys to the kernel, so the webview is sent them too. They
// are acted on by the Rust side; here a key must do nothing at all - no
// scrolling, no browser shortcut - unless it is being typed into a field.
const typing = (target: EventTarget | null) =>
  target instanceof HTMLInputElement ||
  target instanceof HTMLTextAreaElement ||
  (target instanceof HTMLElement && target.isContentEditable);

for (const type of ["keydown", "keypress", "keyup"]) {
  window.addEventListener(type, (event) => typing(event.target) || event.preventDefault(), { capture: true });
}

render(() => <App />, document.getElementById("root")!);
