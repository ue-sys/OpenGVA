import { Index, Show } from "solid-js";
import type { PinPage as Page, Standing } from "./access";
import type { Touched } from "./entry";
import { Keyboard } from "./Keyboard";
import "./access.css";

// How the page stands, in words: colour repeats what the words
// say and is never the only thing saying it.
const STANDING: Record<Standing, string> = {
  entering: "",
  checking: "Checking",
  refused: "PIN Refused",
  waiting: "Wait",
  unchecked: "PIN Not Checked",
};

/** A PIN page: what the crew asked for, whose PIN it needs, a box with a
 *  mark in it for each digit entered - how many, and never which - and the
 *  pad the digits are typed on, which is the form's phone pad. It is a
 *  dialog: F13 cancels it and F20 enters the PIN, and
 *  the buttons down the sides have the pad's digits on them too.
 *
 *  A touch on a key or on an account is told of with `onTouch`, and a
 *  finger's leaving a key with `onLift`; what either does is the frame's to
 *  say. An account is told of as a form's field is: by its number. */
export function PinPage(props: {
  page: Page;
  onTouch?: (event: PointerEvent, touched: Touched) => void;
  onLift?: () => void;
}) {
  const standing = () =>
    props.page.standing === "waiting" && props.page.wait !== null
      ? `${STANDING.waiting} ${props.page.wait} s`
      : STANDING[props.page.standing];

  return (
    <div class="page pin-page">
      <div class="asked">
        <header>
          <h1>{props.page.wanted}</h1>
          <span class={`standing ${props.page.standing}`}>{standing()}</span>
        </header>

        {/* Whose PIN: the one account, or the one chosen of several, each
            of which is a row to touch and is marked as a form's field is
            when the cursor is on it. */}
        <Show
          when={props.page.accounts.length > 0}
          fallback={
            <div class="whose">
              PIN of <span class="account">{props.page.account}</span>
            </div>
          }
        >
          <div class="whose">PIN of</div>
          <div class="accounts">
            <Index each={props.page.accounts}>
              {(row) => (
                <div
                  class={`account${row().chosen ? " chosen" : ""}`}
                  onPointerDown={(event) => props.onTouch?.(event, { kind: "field", number: row().number })}
                >
                  {row().name}
                </div>
              )}
            </Index>
          </div>
        </Show>

        {/* The box is as long as the longest PIN. What is in it says how
            many digits there are, and that is all the page was told. */}
        <div class="entry">
          <span class="box" style={{ "--most": props.page.most }}>
            {"●".repeat(props.page.entered)}
          </span>
          <span class="said">{props.page.said ?? ""}</span>
        </div>
      </div>

      <Keyboard keyboard={props.page.pad} onTouch={props.onTouch} onLift={props.onLift} />
    </div>
  );
}
