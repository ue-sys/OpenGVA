import { For } from "solid-js";
import { degrees } from "./format";

// Plan Position Indicator: a north marker, 32 equally spaced
// marks round the edge, and the vehicle at the centre with its front
// unmistakable. Heading-up: the vehicle points up the screen
// and north turns about it. A threat is a red bar at the edge, the way it
// lies from the vehicle's front;
// the one picked out in the threats table has a white edge to it. Weapon
// and sight indications come later.
export function Ppi(props: { headingMils: number | null; threats: { bearingMils: number; picked: boolean }[] }) {
  const marks = Array.from({ length: 32 }, (_, i) => i);
  // With no heading yet, north is drawn at the top.
  const north = () => -degrees(props.headingMils ?? 0);

  return (
    <svg class="ppi" viewBox="-100 -100 200 200" role="img" aria-label="Plan position indicator">
      <circle r="98" class="ppi-face" />
      <g transform={`rotate(${north()})`}>
        <For each={marks}>
          {(i) => (
            <line
              class="ppi-mark"
              transform={`rotate(${i * 11.25})`}
              x1="0"
              y1={i % 8 === 0 ? -70 : i % 4 === 0 ? -76 : -80}
              x2="0"
              y2="-88"
            />
          )}
        </For>
        <text class="ppi-north" x="0" y="-52" text-anchor="middle">
          N
        </text>
      </g>
      <path class="ppi-vehicle" d="M0 -34 L20 30 L0 16 L-20 30 Z" />
      <For each={props.threats}>
        {(threat) => (
          <rect
            class="ppi-threat"
            classList={{ picked: threat.picked }}
            transform={`rotate(${degrees(threat.bearingMils)})`}
            x="-13"
            y="-98"
            width="26"
            height="8"
          />
        )}
      </For>
    </svg>
  );
}
