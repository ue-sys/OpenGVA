// A PIN page, as hmi-access serialises it (app/crates/hmi-access is the
// definition; this mirrors it).

import type { KeyboardView } from "./entry";

/** How a PIN page stands: a PIN can be entered; one is away being checked;
 *  the last was refused; the verifier will hear none until a wait is over;
 *  or the last could not be checked, there being nobody to check it. */
export type Standing = "entering" | "checking" | "refused" | "waiting" | "unchecked";

/** An account whose PIN would do. */
export interface AccountRow {
  /** Counted from 1: what a touch on it is told of by. */
  number: number;
  name: string;
  /** Whether it is the one whose PIN is being asked for. */
  chosen: boolean;
}

/** A PIN page: what the crew asked for, whose PIN it needs, and the pad the
 *  PIN is typed on. */
export interface PinPage {
  /** What the crew asked for, as they know it. */
  wanted: string;
  /** The account whose PIN is being asked for. */
  account: string;
  /** The accounts whose PIN would do, where there is more than one to choose
   *  between; none, where there is one. */
  accounts: AccountRow[];
  /** How many digits have been entered. Not which: the page never has them. */
  entered: number;
  /** The most there can be: how long the box is. */
  most: number;
  standing: Standing;
  /** What the verifier said, the last time it refused or could not check. */
  said: string | null;
  /** How many seconds it is until the verifier will hear another PIN. */
  wait: number | null;
  /** The phone pad, with nothing highlighted. A key touched is told of by its
   *  place in its `keys`. */
  pad: KeyboardView;
}
