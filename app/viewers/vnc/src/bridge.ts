// The only place the page talks to the Rust side: one command to ask what it
// is to show, one to say that it has the server's screen, one to say that it
// has not. Plain messages - nothing else in the page knows that the
// transport is Tauri.

import { invoke, isTauri } from "@tauri-apps/api/core";

/** What the page is to show, and how (src-tauri/src/main.rs is the
 *  definition; this mirrors it). */
export interface View {
  /** The server, as it is called on the window when its screen is not there. */
  server: string;
  /** Where to connect to reach it: a WebSocket that carries RFB. */
  url: string;
  /** Whether the screen is only to be looked at. */
  viewOnly: boolean;
  /** How the screen is fitted into the window: as large as will go in, or a
   *  pixel to a pixel. */
  scale: "fit" | "none";
  /** The server's password, if it is to be given one. */
  password: string | null;
}

export async function theView(): Promise<View> {
  if (isTauri()) return invoke<View>("view");
  // A plain browser has no Rust side, and so no way to a VNC server but one
  // that is itself a WebSocket. In development the address says where that
  // is, so that the page can be worked on; QEMU has one to offer
  // (-vnc 127.0.0.1:70,websocket=5770), and so has websockify.
  //
  //   ?url=ws://127.0.0.1:5770   the WebSocket
  //   &server=NAME               what the server is called on the window
  //   &view-only=yes             only look
  //   &scale=none                a pixel to a pixel
  //   &password=PASSWORD         for a server that asks for one
  if (import.meta.env.DEV) {
    const said = new URLSearchParams(location.search);
    const url = said.get("url") ?? "ws://127.0.0.1:5770";
    return {
      server: said.get("server") ?? new URL(url).host,
      url,
      viewOnly: said.get("view-only") === "yes",
      scale: said.get("scale") === "none" ? "none" : "fit",
      password: said.get("password"),
    };
  }
  throw new Error("no shell to talk to");
}

// What the shell would put in the log, a browser has only its console for.

export function reportConnected(): void {
  if (isTauri()) void invoke("connected");
  else console.info("connected");
}

/** `why` is for the log: in the words of whichever end said it. */
export function reportDisconnected(why: string): void {
  if (isTauri()) void invoke("disconnected", { why });
  else console.info(`disconnected: ${why}`);
}
