// Touch: what the HMI draws that can be touched says so to the Rust side,
// which decides what the touch does. The UI does nothing of its
// own with one - it keeps no state - and whatever can be touched can also be
// done from the bezel.

import { reportLift, reportTouch } from "./bridge";
import type { Touched } from "./frame";

/** For an element's `onPointerDown`: a finger on it - or the main button of
 *  a mouse, where one stands in for a finger - is a touch on `touched`. It
 *  counts when it lands, as a bezel button does when it goes down. */
export function touch(event: PointerEvent, touched: Touched): void {
  if (event.pointerType === "mouse" && event.button !== 0) return;
  event.preventDefault();
  reportTouch(touched);
}

/** For the `onPointerUp` and `onPointerCancel` of an element that is lit for
 *  as long as it is touched - a key of the on-screen keyboard:
 *  the finger has gone. Nothing else needs to say so: a label, or a row of a
 *  table, did what it does when the finger landed. */
export function lift(): void {
  reportLift();
}
