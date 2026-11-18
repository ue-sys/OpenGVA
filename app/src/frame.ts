// The frame the Rust side hands over, as hmi-core serialises it
// (app/crates/hmi-core/src/lib.rs is the definition; this mirrors it).

import type { PinPage } from "./access";
import type { FormPage, KeyboardView, Touched as FormTouched } from "./entry";

export type AreaState = "selected" | "available" | "unavailable" | "absent";
export type LabelState = "available" | "selected" | "unavailable" | "blank";
export type Category = "warning" | "caution" | "advisory";
/** Where an alarm stands: its condition is there, the crew has overridden
 *  it, or it has gone and only the table still has it. */
export type AlarmState = "active" | "overridden" | "resolved";

export interface AlarmCounts {
  warnings: number;
  cautions: number;
  advisories: number;
}

/** An alarm as the Alarms page lists it. */
export interface Alarm {
  category: Category;
  /** "W – Engine Oil Low Pressure" */
  text: string;
  /** When it was raised: a date-time group, or in the table its day and time. */
  time: string;
  acknowledged: boolean;
  state: AlarmState;
}

/** Where a threat stands: it is there, or it has gone and only the Threats
 *  page still has it. */
export type ThreatState = "current" | "past";
export type Priority = "high" | "medium" | "low";
export type SensorStatus = "ok" | "degraded" | "failed" | "off";

/** A threat as the Threats page lists it. */
export interface Threat {
  /** "T – Laser Rangefinder" */
  text: string;
  /** When it was detected: a date-time group, or in the table its day and time. */
  time: string;
  /** Where it is from the vehicle, as far as is known: mils clockwise from
   *  the vehicle's front... */
  bearingMils: number | null;
  /** ...and metres. */
  rangeM: number | null;
  state: ThreatState;
}

/** A threat on the threats' picture. */
export interface Plotted {
  /** Which threat it is: what a touch on it picks it by. */
  id: string;
  /** What it is drawn as: a MIL-STD-2525C symbol identification code. */
  symbol: string;
  /** What type of threat it is, to be written by its symbol. */
  label: string;
  bearingMils: number;
  /** `null` for a threat known only by its direction, which goes at the edge. */
  rangeM: number | null;
  state: ThreatState;
  picked: boolean;
}

/** How the Threats page shows the threats. */
export type ThreatsView =
  // A table, a page of it at a time. A row's `id` says which threat it is:
  // what a touch on the row picks it by.
  | { kind: "table"; page: number; pages: number; rows: (Threat & { id: string; selected: boolean })[] }
  // The Plan Position Indicator at the size of the page, with the threats on
  // it where they are, out to `rangeM` metres. `elsewhere` is how many more
  // there are that cannot be put on it, for want of a bearing.
  | { kind: "plot"; rangeM: number; threats: Plotted[]; elsewhere: number };

export interface Frame {
  areas: { id: string; state: AreaState }[];
  status: {
    dtg: string;
    position: string | null;
    headingMils: number | null;
    alarms: AlarmCounts;
  };
  /** A line break in a label's text is one on the screen. */
  labels: { key: string; text: string; state: LabelState }[];
  /** The alarm text message that goes directly beneath the status
   *  information bar, when there is one. */
  alarm: { category: Category; text: string; acknowledged: boolean } | null;
  /** The threat text message that goes along the bottom of the page, when
   *  there is one. */
  threat: { text: string; acknowledged: boolean } | null;
  /** The Plan Position Indicator, when it is on the screen, and where the
   *  current threats are on it: mils clockwise from the vehicle's front. */
  ppi: { threats: { bearingMils: number; picked: boolean }[] } | null;
  page: Page;
  /** What F19 has taken off the screen for now, and is not to be drawn: the
   *  labels of the bezel buttons - the bars under the functional-area buttons
   *  and the soft-key labels - and the status information bar. */
  hidden: { labels: boolean; status: boolean };
  /** Whether the touch screen is on. While it is off, a touch does nothing,
   *  here or in another program's picture. */
  touch: boolean;
}

/** Something of the frame that has been touched: a soft-key label, by its
 *  button, or a row of the table on the page - or a threat's symbol on the
 *  threats' picture - by its `id`; or a field of a form's page, or a key of
 *  the on-screen keyboard, a form's or the one up over another program's
 *  page (entry.ts). */
export type Touched = { kind: "label"; key: string } | { kind: "row"; id: string } | FormTouched;

/** A tile: the part of the display that one of several programs' pictures
 *  has, in the pixels of the display at its reference size, 1024 by 768. */
export interface Tile {
  /** What the crew know the picture as: the label of the button that put it
   *  there. */
  label: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export type Page =
  // Drawn by the HMI.
  | { kind: "own"; title: string; rows: { name: string; value: string }[] }
  // Another program's picture, under the HMI's window: the middle of the
  // screen is left clear for it, but for the overlay, an image to draw over
  // it - and for the on-screen keyboard, while the crew has it up to type
  // into the program with: that has the lower part of the page.
  | { kind: "external"; overlay: string | null; keyboard: KeyboardView | null }
  // Other programs' pictures side by side, each under the HMI's window in a
  // tile of its own: the tiles are left clear, and drawn round.
  | { kind: "tiles"; tiles: Tile[] }
  // The Alarms page: the table, a page of it at a time, and over it the
  // picked alarm at length when that has been asked for.
  | {
      kind: "alarms";
      unacknowledged: AlarmCounts;
      acknowledged: AlarmCounts;
      page: number;
      pages: number;
      /** A row's `id` says which alarm it is: what a touch on the row picks
       *  it by. */
      rows: (Alarm & { id: string; selected: boolean })[];
      detail: (Alarm & { detail: string | null }) | null;
    }
  // The Threats page: how many threats there are, how the sensors that look
  // for them are, the threats as a table or as a picture, and over it the
  // picked threat at length when that has been asked for.
  | {
      kind: "threats";
      counts: { current: number; past: number };
      sensors: { name: string; status: SensorStatus }[];
      view: ThreatsView;
      detail:
        | (Threat & { type: string; description: string; priority: Priority; acknowledged: boolean; detail: string | null })
        | null;
    }
  // A form for the crew to fill in and, while they are entering data into
  // it, the on-screen keyboard (entry.ts).
  | ({ kind: "form" } & FormPage)
  // A PIN being asked for, the crew having gone to something that needs a
  // role: a dialog, over whatever the page was (access.ts).
  | ({ kind: "pin" } & PinPage);

/** A rectangle of the window, in CSS pixels. */
export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Where the display is in the window: its top left corner in CSS pixels,
 *  and how many of those one pixel of the display at its reference size is. */
export interface Placed {
  x: number;
  y: number;
  scale: number;
}
