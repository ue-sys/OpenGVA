// MIL-STD-2525C symbols for whatever draws a tactical picture. What draws
// them (milsymbol-2525c.ts) is much the largest thing in the UI and is not
// wanted until such a picture is first looked at, so it is fetched then:
// until it has come, there is no symbol to give.

import { createSignal } from "solid-js";
import type { Drawn } from "./milsymbol-2525c";

const [draw, setDraw] = createSignal<(code: string) => Drawn>();
const drawn = new Map<string, Drawn>();
let asked = false;

/** The symbol a symbol identification code identifies, once it can be drawn. */
export function symbol(code: string): Drawn | undefined {
  if (!asked) {
    asked = true;
    import("./milsymbol-2525c").then((library) => setDraw(() => library.symbol)).catch((e) => console.error(e));
  }
  const library = draw();
  if (!library) return undefined;
  if (!drawn.has(code)) drawn.set(code, library(code));
  return drawn.get(code);
}
