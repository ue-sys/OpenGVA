// The only place the UI talks to the Rust side: one command to fetch the
// frame, one event that brings a new one, one command to say the first has
// been painted, one to say which parts of the window are drawn on, one to
// say what has been touched and one that the finger has gone. Plain messages
// - nothing else in the UI knows that the transport is Tauri.

import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Frame, Placed, Rect, Touched } from "./frame";

/** Calls `draw` with the frame as it stands, and again whenever it changes. */
export async function watchFrames(draw: (frame: Frame) => void): Promise<void> {
  if (isTauri()) {
    // Subscribe before fetching, so that a change in between is not missed.
    await listen<Frame>("frame", (event) => draw(event.payload));
    draw(await invoke<Frame>("frame"));
    return;
  }
  // A plain browser has no Rust side. In development, draw a canned frame so
  // the layout can be worked on; the import is dropped from production builds.
  if (import.meta.env.DEV) {
    draw((await import("./preview")).previewFrame());
    return;
  }
  throw new Error("no HMI core to talk to");
}

export function reportFirstFrame(): void {
  if (isTauri()) void invoke("first_frame");
}

/** Says which parts of the window are the bars and labels, whether a touch
 *  on the rest goes through to another program - whose picture is there, and
 *  the touch screen on - and where in the window the display is, which is
 *  what says where a tile of it is. */
export function reportSolid(rects: Rect[], through: boolean, display: Placed): void {
  if (isTauri()) void invoke("solid", { rects, through, display });
}

/** Says what of the frame has been touched. What that does - if anything -
 *  comes back as a frame. */
export function reportTouch(touched: Touched): void {
  if (isTauri()) void invoke("touch", { touched });
}

/** Says that the finger which touched a key of the on-screen keyboard has
 *  gone: the key is lit until then. */
export function reportLift(): void {
  if (isTauri()) void invoke("lift");
}
