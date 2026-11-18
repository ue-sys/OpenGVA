// A form's page and the on-screen keyboard, as hmi-entry serialises them
// (app/crates/hmi-entry is the definition; this mirrors it).

/** What a field takes: free text, a whole number, or a number with a decimal
 *  point in it. */
export type FieldKind = "text" | "number" | "decimal";

/** Where a form's data stands: nothing saved yet, as it was last saved, or
 *  being entered - with nothing changed so far, or not as it is saved. */
export type Standing = "empty" | "saved" | "entering" | "changed";

/** A field of the form. */
export interface FieldRow {
  /** Which of the form's fields it is, counted from 1: what a touch on it is
   *  told of by. */
  number: number;
  title: string;
  kind: FieldKind;
  /** The most characters it holds: how long its box is. */
  length: number;
  value: string;
  /** Whether the cursor is on it. */
  cursor: boolean;
}

export type KeyKind = "character" | "space" | "backspace" | "caps" | "symbols";

/** A key of the on-screen keyboard. */
export interface Key {
  /** What is written on it: the character it types as the keyboard now is,
   *  or what it does. */
  text: string;
  kind: KeyKind;
  /** Its place on the keyboard's grid: its row and the column it starts in,
   *  both counted from 0... */
  row: number;
  column: number;
  /** ...and how many columns it takes. */
  span: number;
  /** Whether selecting it would do anything now. */
  available: boolean;
  /** Whether the highlight is on it: it is the key Enter selects. */
  highlighted: boolean;
  /** Whether it is being selected now, by Enter or by a finger. */
  pressed: boolean;
  /** Whether it is a key that stays on, and is on: Caps. */
  on: boolean;
}

/** An on-screen keyboard: its keys, each with its place on a grid of
 *  `columns` by `rows`. A key touched is told of by its place in `keys`. */
export interface KeyboardView {
  layout: "qwerty" | "phone";
  columns: number;
  rows: number;
  keys: Key[];
}

/** A form's page: its fields, or while the keyboard is up as many of them as
 *  there is room for beside it, and the keyboard while data is being entered. */
export interface FormPage {
  title: string;
  standing: Standing;
  /** The field the cursor is on, counted from 1... */
  field: number;
  /** ...of this many. */
  fields: number;
  rows: FieldRow[];
  keyboard: KeyboardView | null;
  /** Whether the crew is being asked if what has been changed is to be
   *  discarded: a dialog, in the keyboard's place. */
  discard: boolean;
}

/** Something of a form's page that has been touched: a field, by its number,
 *  or a key, by its place among the keyboard's. */
export type Touched = { kind: "field"; number: number } | { kind: "key"; at: number };
