import { For } from "solid-js";
import type { Tile } from "./frame";

// A page of tiles: other programs' pictures side by side, each where the
// frame has its tile. The pictures are not drawn here. They are windows under
// this one, which is clear wherever this page is, so the page is a mask:
// black all over but for the tiles, and nothing of a window is seen but what
// is in its tile. A tile has a border, and what its picture is called in a
// corner; nothing else of the page's is over a picture.
export function TilesPage(props: { tiles: Tile[] }) {
  // The display, and then each tile: what is in both is left out of the
  // mask ("evenodd").
  const mask = () =>
    "M0 0H1024V768H0Z" + props.tiles.map((t) => `M${t.x} ${t.y}h${t.width}v${t.height}h${-t.width}Z`).join("");

  return (
    <div class="tiles">
      <svg class="tiles-mask" viewBox="0 0 1024 768" aria-hidden="true">
        <path fill-rule="evenodd" d={mask()} />
      </svg>
      <For each={props.tiles}>
        {(tile) => (
          <div
            class="tile"
            style={{ left: `${tile.x}px`, top: `${tile.y}px`, width: `${tile.width}px`, height: `${tile.height}px` }}
          >
            <span class="tile-label">{tile.label}</span>
          </div>
        )}
      </For>
    </div>
  );
}
