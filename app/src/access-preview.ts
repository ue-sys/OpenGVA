// Development only: a PIN page for the HMI's preview (preview.ts), where
// there is no Rust side to ask for one. Never part of a production build.
//
// The address says how the page stands:
//
//   ?pin             a PIN just asked for
//   ?pin=typed       four digits entered, and the 7 being pressed
//   ?pin=checking    the PIN away being checked
//   ?pin=refused     the last PIN refused
//   ?pin=waiting     the verifier not to be asked for a while
//   ?pin=unchecked   nobody there to check a PIN
//   ?pin=several     three accounts, any of whose PIN would do
//
// What preview.ts says of the rest of the frame goes with any of them: an
// alarm's text message and a threat's, neither of which waits on a PIN,
// what F19 has hidden, the touch screen off.

import type { PinPage, Standing } from "./access";
import type { Key, KeyKind } from "./entry";
import type { Frame, LabelState } from "./frame";

const COMMON = ["Cancel", "Alarms", "Threats", "Ack", "Up", "Down", "Labels", "Enter"];

/** `frame`, with a PIN asked for over whatever its page was: as `asked`. */
export function pinFrame(frame: Frame, asked: string): Frame {
  const standing: Standing =
    ({ checking: "checking", refused: "refused", waiting: "waiting", unchecked: "unchecked" } as Record<string, Standing>)[
      asked
    ] ?? "entering";
  const entered = { typed: 4, several: 2 }[asked] ?? 0;
  const several = asked === "several";
  // What can be typed: nothing while the PIN is away, or the verifier is
  // waited for.
  const typing = standing !== "checking" && standing !== "waiting";

  // The phone pad as hmi-entry lays it out (keyboard.rs has the same rows),
  // with nothing highlighted: Enter enters the PIN.
  const key = (text: string, kind: KeyKind, row: number, column: number, available: boolean): Key => ({
    text,
    kind,
    row,
    column,
    span: 1,
    available,
    highlighted: false,
    pressed: asked === "typed" && text === "7",
    on: false,
  });
  const keys: Key[] = [
    ...["123", "456", "789"].flatMap((digits, row) =>
      [...digits].map((digit, column) => key(digit, "character", row, column, typing)),
    ),
    key(".", "character", 3, 0, false),
    key("0", "character", 3, 1, typing),
    key("Backspace", "backspace", 3, 2, typing && entered > 0),
  ];

  const page: PinPage = {
    wanted: "COM: Net Settings",
    account: several ? "jones" : "commander",
    accounts: several
      ? ["smith", "jones", "commander"].map((name, i) => ({ number: i + 1, name, chosen: name === "jones" }))
      : [],
    entered,
    most: 12,
    standing,
    said:
      {
        refused: "not the account's PIN",
        waiting: "The account is locked due to 5 failed logins. (1 minute left to unlock)",
        unchecked: "no verifier is running",
      }[asked] ?? null,
    wait: standing === "waiting" ? 58 : null,
    pad: { layout: "phone", columns: 3, rows: 4, keys },
  };

  // The labels as hmi-access gives them: the pad's digits down the sides and
  // Backspace under them; of the common functions F13 cancels, F20 enters,
  // and F17 and F18 choose between accounts where there are several.
  const able = (can: boolean): LabelState => (can ? "available" : "unavailable");
  const sides: [string, LabelState][] = [
    ...["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"].map((digit): [string, LabelState] => [digit, able(typing)]),
    ["Backspace", able(typing && entered > 0)],
    ["", "blank"],
  ];
  const states: Record<number, LabelState> = { 17: able(several), 18: able(several), 20: able(typing && entered >= 4) };
  return {
    ...frame,
    labels: Array.from({ length: 20 }, (_, i) => {
      const [text, state] = sides[i] ?? [COMMON[i - 12], states[i + 1] ?? "available"];
      return { key: `F${i + 1}`, text, state };
    }),
    page: { kind: "pin", ...page },
  };
}
