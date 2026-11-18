import { Index, Show } from "solid-js";
import type { FormPage, Standing, Touched } from "./entry";
import { Keyboard } from "./Keyboard";
import "./entry.css";

// Where the form's data stands, in words: colour repeats what
// the word says and is never the only thing saying it.
const STANDING: Record<Standing, string> = {
  empty: "",
  saved: "Saved",
  entering: "No Changes",
  changed: "Changes Not Saved",
};

/** A form's page: its title, its fields -
 *  each a title and a box to enter into - and, while data
 *  is being entered, the on-screen keyboard under them; or in the keyboard's
 *  place the question that is asked before what has been changed is
 *  discarded.
 *
 *  A touch on a field or on a key is told of with `onTouch`, and a finger's
 *  leaving a key with `onLift`; what either does is the frame's to say. */
export function EntryPage(props: {
  page: FormPage;
  onTouch?: (event: PointerEvent, touched: Touched) => void;
  onLift?: () => void;
}) {
  const entering = () => props.page.keyboard !== null;

  return (
    <div class="page form-page">
      <div class="form">
        <header>
          <h1>{props.page.title}</h1>
          <span class={`standing ${props.page.standing}`}>{STANDING[props.page.standing]}</span>
          <span class="where">
            Field {props.page.field} of {props.page.fields}
          </span>
        </header>

        <Index each={props.page.rows}>
          {(row) => (
            <div
              class={`field ${row().kind}${row().cursor ? " cursor" : ""}`}
              onPointerDown={(event) => props.onTouch?.(event, { kind: "field", number: row().number })}
            >
              <span class="title">{row().title}</span>
              {/* The box is as long as what it can hold. The caret is where
                  the next character goes, in the field being entered into. */}
              <span class="value" style={{ "--length": row().length }}>
                {row().value}
                <span class={row().cursor && entering() ? "caret shown" : "caret"} />
              </span>
            </div>
          )}
        </Index>
      </div>

      <Show when={props.page.keyboard}>
        {(keyboard) => <Keyboard keyboard={keyboard()} onTouch={props.onTouch} onLift={props.onLift} />}
      </Show>

      {/* A dialog: the two buttons that answer it are the two that answer
          any dialog, by the names their labels have. */}
      <Show when={props.page.discard}>
        <div class="discard">
          <h2>Discard Changes?</h2>
          <dl>
            <dt>Enter</dt>
            <dd>Leave data entry. What has been changed is lost.</dd>
            <dt>Cancel</dt>
            <dd>Go on entering.</dd>
          </dl>
        </div>
      </Show>
    </div>
  );
}
