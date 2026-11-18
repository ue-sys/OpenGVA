// Development only: a form's page for the HMI's preview (preview.ts), where
// there is no Rust side to say what one holds. Never part of a production
// build.
//
// The address says which page:
//
//   ?form                a form nobody has filled in
//   ?form=saved          the same, filled in and saved
//   ?form=begin          data entry just begun on the field of free text
//   ?form=text           free text being entered on the QWERTY keyboard
//   ?form=pressed        the same, with the key the highlight is on selected
//   ?form=small          the same, with Caps off
//   ?form=symbols        the same, with the symbols showing
//   ?form=full           the same, with the field holding all it can
//   ?form=discard        the same, asked whether the changes are to be lost
//   ?form=empty          data entry just begun on the number
//   ?form=number         a whole number being entered on the phone pad
//   ?form=many           a form of as many fields as a form can have
//   ?form=many-entering  the same, with the keyboard up
//   ?form=decimal        the same, on a number that has its decimal point
//
// What preview.ts says of the rest of the frame goes with any of them: an
// alarm's text message and a threat's, what F19 has hidden, the touch screen
// off.
//
// And the keyboard over another program's page, which goes with "?external":
//
//   &keyboard            up, in small letters
//   &keyboard=pressed    the same, with the key the highlight is on selected
//   &keyboard=down       put away: the button that brings it up
//   &keyboard=pad        up by itself for a field that takes a whole number

import type { FieldRow, FormPage, Key, KeyKind, KeyboardView } from "./entry";
import type { Frame, LabelState } from "./frame";

const COMMON = ["Up", "Alarms", "Threats", "Ack", "Up", "Down", "Labels", "Enter"];

// The keyboards as hmi-entry lays them out (keyboard.rs has the same rows).
const key = (text: string, kind: KeyKind, row: number, column: number, span: number): Key => ({
  text,
  kind,
  row,
  column,
  span,
  available: true,
  highlighted: false,
  pressed: false,
  on: false,
});
const typing = (row: number, column: number, span: number, characters: string): Key[] =>
  [...characters].map((character, i) => key(character, "character", row, column + i * span, span));

function qwerty(letters: "capitals" | "small" | "symbols"): Key[] {
  const small = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];
  const rows =
    letters === "symbols"
      ? ["!?'\"()&:;@", "+=*%#_<>$", "[]{}\\|~"]
      : letters === "capitals"
        ? small.map((row) => row.toUpperCase())
        : small;
  return [
    ...typing(0, 0, 2, "1234567890"),
    ...typing(1, 0, 2, rows[0]),
    ...typing(2, 1, 2, rows[1]),
    { ...key("Caps", "caps", 3, 0, 3), available: letters !== "symbols", on: letters === "capitals" },
    ...typing(3, 3, 2, rows[2]),
    key("Backspace", "backspace", 3, 17, 3),
    key(letters === "symbols" ? "ABC" : "#+=", "symbols", 4, 0, 3),
    key("Space", "space", 4, 3, 9),
    ...typing(4, 12, 2, ",.-/"),
  ];
}

const phone = (): Key[] => [
  ...typing(0, 0, 1, "123"),
  ...typing(1, 0, 1, "456"),
  ...typing(2, 0, 1, "789"),
  ...typing(3, 0, 1, ".0"),
  key("Backspace", "backspace", 3, 2, 1),
];

/** A keyboard typing into `field`: the highlight on the key that says
 *  `highlighted`, and that key selected if `pressed`. */
function keyboard(keys: Key[], field: FieldRow, highlighted: string, pressed = false): KeyboardView {
  const pad = keys.length === 12;
  const room = field.value.length < field.length;
  // The pad's point is for a number that can have one, and has none yet.
  const takes = (k: Key) =>
    k.kind === "backspace"
      ? field.value.length > 0
      : k.kind === "character" || k.kind === "space"
        ? room && (!pad || k.text !== "." || (field.kind === "decimal" && !field.value.includes(".")))
        : true;
  return {
    layout: pad ? "phone" : "qwerty",
    columns: pad ? 3 : 20,
    rows: pad ? 4 : 5,
    keys: keys.map((k) => ({
      ...k,
      available: k.available && takes(k),
      highlighted: k.text === highlighted,
      pressed: pressed && k.text === highlighted,
    })),
  };
}

// A form of a free-text field and a number, and one of as many fields as
// a form can have, with titles and values as long as they come.
const row = (number: number, title: string, kind: FieldRow["kind"], length: number, value = ""): FieldRow => ({
  number,
  title,
  kind,
  length,
  value,
  cursor: false,
});
const load = (description: string, quantity: string): FieldRow[] => [
  row(1, "Description", "text", 24, description),
  row(2, "Quantity", "number", 4, quantity),
];
const many = (frequency: string): FieldRow[] => [
  row(1, "Unit", "text", 12, "2 TROOP"),
  row(2, "Call Sign", "text", 6, "T22A"),
  row(3, "Grid Reference", "text", 14, "30U XC 123 456"),
  row(4, "Number Of Vehicles", "number", 3, "4"),
  row(5, "Fuel State (Percent Of Full)", "number", 3, "75"),
  row(6, "Remarks", "text", 28, "NOTHING SEEN SINCE LAST REPT"),
  row(7, "Frequency (MHz)", "decimal", 7, frequency),
  row(8, "Report Time", "text", 7, "041230Z"),
];

type Entering = { keys: Key[]; highlighted: string; pressed?: boolean } | "discard";

function form(
  title: string,
  rows: FieldRow[],
  cursor: number,
  standing: FormPage["standing"],
  entering?: Entering,
): FormPage {
  // With the keyboard up there is room for two: the field being entered into
  // and the next, or at the end of the form the last two.
  const first = entering ? Math.min(cursor - 1, Math.max(0, rows.length - 2)) : 0;
  const on = rows[cursor - 1];
  return {
    title,
    standing,
    field: cursor,
    fields: rows.length,
    rows: rows.slice(first, entering ? first + 2 : undefined).map((r) => ({ ...r, cursor: r.number === cursor })),
    keyboard:
      entering && entering !== "discard" ? keyboard(entering.keys, on, entering.highlighted, entering.pressed) : null,
    discard: entering === "discard",
  };
}

const capitals = (highlighted: string, pressed = false): Entering => ({ keys: qwerty("capitals"), highlighted, pressed });
const pad = (highlighted: string): Entering => ({ keys: phone(), highlighted });

const PAGES: Record<string, () => FormPage> = {
  "": () => form("Load Record", load("", ""), 1, "empty"),
  saved: () => form("Load Record", load("WATER 20L", "48"), 2, "saved"),
  begin: () => form("Load Record", load("", ""), 1, "entering", capitals("G")),
  text: () => form("Load Record", load("WATER 20", ""), 1, "changed", capitals("L")),
  pressed: () => form("Load Record", load("WATER 20L", ""), 1, "changed", capitals("L", true)),
  small: () =>
    form("Load Record", load("Water 20", ""), 1, "changed", { keys: qwerty("small"), highlighted: "Caps" }),
  symbols: () =>
    form("Load Record", load("WATER 20L", ""), 1, "changed", { keys: qwerty("symbols"), highlighted: "ABC" }),
  full: () => form("Load Record", load("WATER 20L JERRYCANS X 12", ""), 1, "changed", capitals("2")),
  discard: () => form("Load Record", load("WATER 20L", ""), 1, "changed", "discard"),
  empty: () => form("Load Record", load("WATER 20L", ""), 2, "changed", pad("5")),
  number: () => form("Load Record", load("WATER 20L", "48"), 2, "changed", pad("8")),
  many: () => form("Situation Report", many("45.250"), 5, "saved"),
  "many-entering": () => form("Situation Report", many("45.250"), 5, "entering", pad("5")),
  decimal: () => form("Situation Report", many("45.2"), 7, "changed", pad("2")),
};

/** The labels as hmi-entry gives them, over the common functions' own. */
function labels(page: FormPage): Frame["labels"] {
  const entering = page.keyboard !== null;
  const on = page.rows.find((r) => r.cursor)!;
  const able = (can: boolean): LabelState => (can && !page.discard ? "available" : "unavailable");
  const sides: Record<number, [string, LabelState]> = {
    1: ["Previous\nField", able(page.field > 1)],
    6: ["Left", able(entering)],
    7: ["Next\nField", able(page.field < page.fields)],
    8: ["Save", able(page.standing === "changed")],
    10: ["Space", able(entering && on.kind === "text" && on.value.length < on.length)],
    11: ["Backspace", able(entering && on.value.length > 0)],
    12: ["Right", able(entering)],
  };
  return Array.from({ length: 20 }, (_, i) => {
    const [text, state] = sides[i + 1] ?? (i < 12 ? ["", "blank"] : [COMMON[i - 12], "available"]);
    // F13 cancels while data is being entered.
    return { key: `F${i + 1}`, text: i === 12 && (entering || page.discard) ? "Cancel" : text, state };
  });
}

/** `frame`, whose page is another program's, as one of a program that can be
 *  typed into: with the on-screen keyboard up over it, or as `asked`. */
export function typedFrame(frame: Frame, asked: string): Frame {
  if (frame.page.kind !== "external") return frame;
  const up = asked !== "down";
  const pad = asked === "pad";
  const sides: Record<number, string> = up
    ? { 1: "Previous\nField", 5: "Keyboard", 6: "Left", 7: "Next\nField", 8: "Return", 10: "Space", 11: "Backspace", 12: "Right" }
    : { 5: "Keyboard" };
  const home = pad ? "5" : "g";
  const keys = (pad ? phone().map((k) => ({ ...k, available: k.text !== "." })) : qwerty("small")).map((k) => ({
    ...k,
    highlighted: k.text === home,
    pressed: asked === "pressed" && k.text === home,
  }));
  return {
    ...frame,
    labels: frame.labels.map((label, i) =>
      i === 12 && up
        ? { ...label, text: "Cancel" }
        : sides[i + 1] === undefined
          ? label
          : {
              ...label,
              text: sides[i + 1],
              state: (i === 4 && up ? "selected" : i === 9 && pad ? "unavailable" : "available") as LabelState,
            },
    ),
    page: {
      ...frame.page,
      keyboard: !up ? null : pad ? { layout: "phone", columns: 3, rows: 4, keys } : { layout: "qwerty", columns: 20, rows: 5, keys },
    },
  };
}

/** `frame`, with STR selected and its page a form's: the one `asked` for. */
export function formFrame(frame: Frame, asked: string): Frame {
  const page = (PAGES[asked] ?? PAGES[""])();
  return {
    ...frame,
    areas: frame.areas.map((area) =>
      area.id === "STR" ? { ...area, state: "selected" } : area.state === "selected" ? { ...area, state: "available" } : area,
    ),
    labels: labels(page),
    page: { kind: "form", ...page },
  };
}
