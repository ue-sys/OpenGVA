import { Index, Show } from "solid-js";
import type { Key, KeyboardView, Touched } from "./entry";
import "./entry.css";

/** The on-screen keyboard: the keys the
 *  frame lists, each where the frame puts it. A key is a soft key and is
 *  drawn as one: green, grey if it would do nothing, with
 *  a selected label's yellow border if the highlight is on it - that is the
 *  key Enter selects - and lit for as long as it is being selected.
 *
 *  Nothing is kept here. A finger on a key is told of with `onTouch`, and
 *  its going with `onLift`; the frame that comes back says what that did,
 *  and it is the frame that lights a key. */
export function Keyboard(props: {
  keyboard: KeyboardView;
  onTouch?: (event: PointerEvent, touched: Touched) => void;
  onLift?: () => void;
}) {
  // The keys stay where they are from one frame to the next, and only what
  // is on them changes: each is drawn once, and kept. A key's classes are
  // one string, as a soft-key label's are (App.tsx): one that is kept is
  // not to lose some of them when the others change.
  const classes = (key: Key) =>
    [
      "key",
      key.kind,
      !key.available && "unavailable",
      key.highlighted && "highlighted",
      key.pressed && "pressed",
      key.on && "on",
    ]
      .filter(Boolean)
      .join(" ");

  return (
    <div
      class={`keyboard ${props.keyboard.layout}`}
      style={{ "--columns": props.keyboard.columns, "--rows": props.keyboard.rows }}
    >
      <Index each={props.keyboard.keys}>
        {(key, at) => (
          <div
            class={classes(key())}
            style={{ "grid-row": `${key().row + 1}`, "grid-column": `${key().column + 1} / span ${key().span}` }}
            onPointerDown={(event) => {
              // The key has the finger until it is lifted, wherever it has
              // slid to by then: its going is told of once, and here.
              event.currentTarget.setPointerCapture(event.pointerId);
              props.onTouch?.(event, { kind: "key", at });
            }}
            onPointerUp={() => props.onLift?.()}
            onPointerCancel={() => props.onLift?.()}
          >
            <Show when={key().kind === "backspace"} fallback={key().text}>
              <BackspaceMark name={key().text} />
            </Show>
          </div>
        )}
      </Index>
    </div>
  );
}

/** Backspace, as keyboards mark it: what is to its left is rubbed out. The
 *  standard's own examples have an arrow on the key and no word. */
function BackspaceMark(props: { name: string }) {
  return (
    <svg class="backspace-mark" viewBox="0 0 44 26" role="img" aria-label={props.name}>
      <path d="M14 2H42V24H14L2 13Z" />
      <path d="M22 8L32 18M32 8L22 18" />
    </svg>
  );
}
