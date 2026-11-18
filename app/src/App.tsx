import { createEffect, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { AlarmsPage } from "./AlarmsPage";
import { reportFirstFrame, reportSolid, watchFrames } from "./bridge";
import { EntryPage } from "./EntryPage";
import { mils } from "./format";
import type { Frame, Rect } from "./frame";
import { Keyboard } from "./Keyboard";
import { PinPage } from "./PinPage";
import { Ppi } from "./Ppi";
import { ThreatsPage } from "./ThreatsPage";
import { TilesPage } from "./TilesPage";
import { lift, touch } from "./touch";

// The layout is drawn at the reference display size and scaled to whatever
// screen it finds itself on, keeping 4:3.
const WIDTH = 1024;
const HEIGHT = 768;

export function App() {
  const [frame, setFrame] = createSignal<Frame>();
  const [scale, setScale] = createSignal(1);

  const fit = () => setScale(Math.min(window.innerWidth / WIDTH, window.innerHeight / HEIGHT));

  onMount(() => {
    fit();
    window.addEventListener("resize", fit);
    onCleanup(() => window.removeEventListener("resize", fit));
    watchFrames(setFrame).catch((e) => console.error(e));
  });

  // Tell the Rust side once the first frame is on the glass: two animation
  // frames after the data arrived, the browser has painted it.
  let reported = false;
  createEffect(() => {
    if (frame() && !reported) {
      reported = true;
      requestAnimationFrame(() => requestAnimationFrame(reportFirstFrame));
    }
  });

  // The bars and labels stay put whatever the page: say where they are, so
  // that another program's window can be fitted in what they leave - which is
  // more when F19 has hidden them. And where the page is such a window, or
  // several in tiles, this one is clear there to show them, and a touch there
  // is that program's unless the touch screen is off. Where the tiles are is
  // the frame's to say, in the pixels of the display: say where the display
  // is, for those to be found by.
  let display!: HTMLDivElement;
  let said = "";
  createEffect(() => {
    const drawn = frame();
    if (!drawn) return;
    const page = drawn.page;
    const clear = page.kind === "external" || page.kind === "tiles";
    const through = clear && drawn.touch;
    scale(); // the parts move when the display is rescaled
    document.documentElement.classList.toggle("see-through", clear);
    requestAnimationFrame(() => {
      const rects = [...display.querySelectorAll(".solid")].map(bounds).concat(margins(display));
      const box = display.getBoundingClientRect();
      const placed = { x: box.left, y: box.top, scale: box.width / WIDTH };
      // Said again when the window changes size, though it be the same.
      const saying = JSON.stringify([window.innerWidth, window.innerHeight, through, rects, placed]);
      if (saying !== said) {
        said = saying;
        reportSolid(rects, through, placed);
      }
    });
  });

  return (
    <div class="display" ref={display} style={{ transform: `scale(${scale()})` }}>
      <Show when={frame()}>{(f) => <Screen frame={f()} />}</Show>
    </div>
  );
}

/** The whole pixels an element covers any part of. */
function bounds(element: Element): Rect {
  const box = element.getBoundingClientRect();
  const x = Math.floor(box.left);
  const y = Math.floor(box.top);
  return { x, y, width: Math.ceil(box.right) - x, height: Math.ceil(box.bottom) - y };
}

/** What the window has round the display, on a screen that is not 4:3. */
function margins(display: Element): Rect[] {
  const box = bounds(display);
  const right = box.x + box.width;
  const bottom = box.y + box.height;
  return [
    { x: 0, y: 0, width: box.x, height: window.innerHeight },
    { x: right, y: 0, width: window.innerWidth - right, height: window.innerHeight },
    { x: box.x, y: 0, width: box.width, height: box.y },
    { x: box.x, y: bottom, width: box.width, height: window.innerHeight - bottom },
  ].filter((rect) => rect.width > 0 && rect.height > 0);
}

function Screen(props: { frame: Frame }) {
  const label = (n: number) => props.frame.labels[n - 1];
  const range = (from: number, to: number) => Array.from({ length: to - from + 1 }, (_, i) => from + i);
  const status = () => props.frame.status;
  const own = () => (props.frame.page.kind === "own" ? props.frame.page : undefined);
  const external = () => (props.frame.page.kind === "external" ? props.frame.page : undefined);
  const tiles = () => (props.frame.page.kind === "tiles" ? props.frame.page : undefined);
  const alarms = () => (props.frame.page.kind === "alarms" ? props.frame.page : undefined);
  const threats = () => (props.frame.page.kind === "threats" ? props.frame.page : undefined);
  const form = () => (props.frame.page.kind === "form" ? props.frame.page : undefined);
  const pin = () => (props.frame.page.kind === "pin" ? props.frame.page : undefined);
  // The on-screen keyboard, while it is up over another program's page.
  const typing = () => external()?.keyboard ?? undefined;

  // What F19 has hidden is not drawn, and gives up its room.
  const hidden = () => props.frame.hidden;

  // Elements of class "solid" are the bars and labels: App reports them.
  return (
    <div
      class="screen"
      classList={{
        external: external() !== undefined || tiles() !== undefined,
        typing: typing() !== undefined,
        "no-labels": hidden().labels,
        "no-status": hidden().status,
      }}
    >
      {/* Over the other program's picture, under everything else of the HMI's. */}
      <Show when={external()?.overlay}>{(image) => <img class="overlay" src={image()} alt="" />}</Show>
      <Show when={tiles()}>{(page) => <TilesPage tiles={page().tiles} />}</Show>

      {/* The on-screen keyboard over another program's page:
          a band across the bottom of it, from edge to edge and solid, so
          that the program's window is fitted into what is left above and
          all of its picture is still to be seen. While it is up the columns
          of labels beside the picture are solid too: the picture is between
          them, with no label lying over it. */}
      <Show when={typing()}>
        {(keyboard) => (
          <div class="keyboard-band solid">
            <Keyboard keyboard={keyboard()} onTouch={touch} onLift={lift} />
          </div>
        )}
      </Show>

      {/* One bar under each of the eight top bezel buttons. */}
      <Show when={!hidden().labels}>
        <div class="areas solid">
          <For each={props.frame.areas}>{(a) => <div class={`area ${a.state}`} title={a.id} />}</For>
        </div>
      </Show>

      <Show when={!hidden().status}>
        <div class="status solid">
          <span class="dtg">{status().dtg}</span>
          <span class="position">{status().position ?? "NO POSITION"}</span>
          <span class="heading">{status().headingMils === null ? "---- mils" : mils(status().headingMils!)}</span>
          <span class="alarms">
            <span class="warnings">W{status().alarms.warnings}</span>
            <span class="cautions">C{status().alarms.cautions}</span>
            <span class="advisories">A{status().alarms.advisories}</span>
          </span>
        </div>
      </Show>

      {/* An alarm's text message: directly beneath the status information
          bar, over whatever the page is. */}
      <Show when={props.frame.alarm}>
        {(alarm) => (
          <div class={`alarm solid ${alarm().category}${alarm().acknowledged ? " acknowledged" : ""}`}>
            <div class="text">{alarm().text}</div>
          </div>
        )}
      </Show>

      {/* A threat's text message: along the bottom of the page, where an
          alarm's is not, so that both can be read at once. */}
      <Show when={props.frame.threat}>
        {(threat) => (
          <div class="threat solid" classList={{ acknowledged: threat().acknowledged }}>
            <div class="text">{threat().text}</div>
          </div>
        )}
      </Show>

      <Show when={!hidden().labels}>
        <div class={`labels left${typing() ? " solid" : ""}`}>
          <For each={range(1, 6)}>{(n) => <SoftKey label={label(n)} />}</For>
        </div>
      </Show>

      <Show when={alarms()}>{(page) => <AlarmsPage page={page()} />}</Show>
      <Show when={threats()}>{(page) => <ThreatsPage page={page()} headingMils={status().headingMils} />}</Show>
      <Show when={form()}>{(page) => <EntryPage page={page()} onTouch={touch} onLift={lift} />}</Show>
      <Show when={pin()}>{(page) => <PinPage page={page()} onTouch={touch} onLift={lift} />}</Show>

      {/* The Plan Position Indicator, top left of the page,
          over whatever the page is: the frame says when it is there. */}
      <Show when={props.frame.ppi}>{(ppi) => <Ppi headingMils={status().headingMils} threats={ppi().threats} />}</Show>

      <Show when={own()}>
        {(page) => (
          <div class="page">
            <h1>{page().title}</h1>
            <table>
              <tbody>
                <For each={page().rows}>
                  {(row) => (
                    <tr>
                      <th>{row.name}</th>
                      <td>{row.value}</td>
                    </tr>
                  )}
                </For>
              </tbody>
            </table>
          </div>
        )}
      </Show>

      <Show when={!hidden().labels}>
        <div class={`labels right${typing() ? " solid" : ""}`}>
          <For each={range(7, 12)}>{(n) => <SoftKey label={label(n)} />}</For>
        </div>

        <div class="labels bottom solid">
          <For each={range(13, 20)}>{(n) => <SoftKey label={label(n)} />}</For>
        </div>
      </Show>
    </div>
  );
}

/** A soft-key label, which is a soft key too: touched, it is its button.
 *  One with nothing on it is not there to be touched.
 *
 *  Its classes are one string. A `class` that changes replaces them all,
 *  and "solid" beside it in a `classList` would be gone the first time a
 *  label went from one state to another - and the label, over another
 *  program's picture, with it: no longer the HMI's to be touched. */
function SoftKey(props: { label: Frame["labels"][number] }) {
  return (
    <div
      class={`softkey ${props.label.state}${props.label.state === "blank" ? "" : " solid"}`}
      data-key={props.label.key}
      onPointerDown={(event) => props.label.state !== "blank" && touch(event, { kind: "label", key: props.label.key })}
    >
      {props.label.text}
    </div>
  );
}
