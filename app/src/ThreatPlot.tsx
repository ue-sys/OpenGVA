import { For, Show } from "solid-js";
import { degrees, metres } from "./format";
import type { Plotted } from "./frame";
import { symbol } from "./symbols";
import { touch } from "./touch";

// The threats' picture: the Plan Position Indicator at the size of the page.
// The vehicle is at the centre with its front up the screen,
// as on the indicator in the corner; rings mark off the range, and each
// threat is where it is from the vehicle, drawn as the symbol MIL-STD-2525C
// gives it with its type written beside. A threat known only by its
// direction is put outside the rings, at the edge.

// In the drawing's own units, of which the face is 100 across the radius:
const FULL = 78; // the ring at the picture's full range
const EDGE = 88; // where a threat of no known range goes
const SCALE = 0.26; // a symbol's size, of the size it is drawn at
const RINGS = [0.5, 1];

/** Where on the drawing something `bearing` mils round from the vehicle's
 *  front and `out` units from it is. */
function place(bearing: number, out: number) {
  const angle = (degrees(bearing) * Math.PI) / 180;
  return { x: out * Math.sin(angle), y: -out * Math.cos(angle) };
}

export function ThreatPlot(props: { headingMils: number | null; rangeM: number; threats: Plotted[] }) {
  const marks = Array.from({ length: 32 }, (_, i) => i);
  const north = () => -degrees(props.headingMils ?? 0);
  const at = (threat: Plotted) =>
    place(threat.bearingMils, threat.rangeM === null ? EDGE : (Math.min(threat.rangeM, props.rangeM) / props.rangeM) * FULL);

  return (
    <svg class="threat-plot" viewBox="-100 -100 200 200" role="img" aria-label="Threats about the vehicle">
      <circle r="98" class="ppi-face" />
      <g transform={`rotate(${north()})`}>
        <For each={marks}>
          {(i) => (
            <line
              class="plot-mark"
              transform={`rotate(${i * 11.25})`}
              x1="0"
              y1={i % 8 === 0 ? -91 : -94}
              x2="0"
              y2="-97.5"
            />
          )}
        </For>
        <text class="plot-north" x="0" y="-83" text-anchor="middle">
          N
        </text>
      </g>

      {/* The rings, each with how far out it is, where the fewest threats
          are: behind and to one side. */}
      <For each={RINGS}>
        {(part) => {
          const label = place(2800, part * FULL);
          return (
            <>
              <circle r={part * FULL} class="plot-ring" />
              <text class="plot-range" x={label.x + 1.5} y={label.y + 6}>
                {metres(props.rangeM * part)}
              </text>
            </>
          );
        }}
      </For>
      <path class="plot-vehicle" d="M0 -34 L20 30 L0 16 L-20 30 Z" transform="scale(0.2)" />

      <For each={props.threats}>
        {(threat) => {
          const where = at(threat);
          return (
            <g
              class={`plot-threat ${threat.state}${threat.picked ? " picked" : ""}`}
              onPointerDown={(event) => touch(event, { kind: "row", id: threat.id })}
            >
              {/* A symbol is small to put a finger on: what is touched is
                  the ground round it. */}
              <circle class="plot-touch" cx={where.x} cy={where.y} r="10.5" />
              <Show
                when={symbol(threat.symbol)}
                // What draws symbols has not come yet: a mark where the symbol will be.
                fallback={<circle cx={where.x} cy={where.y} r="2" class="plot-pending" />}
              >
                {(drawn) => (
                  <image
                    href={drawn().url}
                    x={where.x - drawn().anchor.x * SCALE}
                    y={where.y - drawn().anchor.y * SCALE}
                    width={drawn().width * SCALE}
                    height={drawn().height * SCALE}
                  />
                )}
              </Show>
              <Show when={threat.picked}>
                <rect class="plot-pick" x={where.x - 10.5} y={where.y - 10.5} width="21" height="21" />
              </Show>
              <text class="plot-label" x={where.x + 12} y={where.y + 1.8}>
                {threat.label}
              </text>
            </g>
          );
        }}
      </For>
    </svg>
  );
}
