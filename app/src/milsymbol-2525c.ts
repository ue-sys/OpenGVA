// The symbols of MIL-STD-2525C, drawn by milsymbol from their symbol
// identification codes. The library also knows APP-6 and the other issues of
// MIL-STD-2525, and what it draws a code as depends on which of them it has
// been given: here it is given 2525C and nothing else.

import { ms, std2525c } from "milsymbol/index.mjs";

ms.addIcons(std2525c);
ms.setStandard("2525");

/** A symbol as a picture, and where in the picture the thing it stands for is. */
export interface Drawn {
  url: string;
  width: number;
  height: number;
  anchor: { x: number; y: number };
}

// Hostile, on the ground, there now, and nothing more known of it.
const USUAL = "SHGP-----------";

// The symbols are for a black background, where a symbol's own black
// outline would be lost: each has a white one round it.
const draw = (code: string) => new ms.Symbol(code, { size: 40, outlineWidth: 3, outlineColor: "rgb(255, 255, 255)" });

/** The symbol a code identifies. A code that identifies none is drawn as
 *  what its first four characters say - whose it is and where - and failing
 *  that as a hostile track on the ground. */
export function symbol(code: string): Drawn {
  const symbol = [code, code.slice(0, 4).padEnd(15, "-"), USUAL].map(draw).find((drawn) => drawn.isValid())!;
  return { url: symbol.toDataURL(), ...symbol.getSize(), anchor: symbol.getAnchor() };
}
