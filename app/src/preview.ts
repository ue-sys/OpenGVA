// Development only: a frame for `npm run dev` in a plain browser, where there
// is no Rust side. Never part of a production build (see bridge.ts).
//
// The address says which frame:
//
//   ?external           an area that is another program's window
//   ?external=URL       the same, with the image at URL drawn over it
//   &keyboard           the same, as a program that can be typed into, with
//                       the on-screen keyboard up over it; "&keyboard=down"
//                       and "&keyboard=pressed" are in entry-preview.ts
//   ?sa                 an area that is several programs' windows in tiles,
//                       with none picked
//   ?sa=3               the same with three picked: one to four
//   ?alarms             the Alarms page
//   ?alarms=detail      the same, with an alarm shown at length
//   ?alarms=full        the same, with as much in the table as a page of it
//                       is ever asked to hold
//   &message=warning    with an alarm's text message: "warning",
//                       "acknowledged" (a warning that has been), "caution"
//                       or "long" (a warning with more to say than a line holds)
//   ?threats            the Threats page, as a table
//   ?threats=detail     the same, with a threat shown at length
//   ?threats=full       the same, with as much in the table as a page of it
//                       is ever asked to hold
//   ?threats=plot       the Threats page, as a picture
//   ?form               a form's page; "?form=text", "?form=number" and the
//                       rest of them are listed in entry-preview.ts
//   ?pin                a PIN being asked for; "?pin=typed", "?pin=waiting"
//                       and the rest of them are listed in access-preview.ts
//   &threat             with a threat's text message, and the threats marked
//                       on the Plan Position Indicator; "&threat=acknowledged"
//                       for a threat that has been
//   &hidden=labels      with what F19 hides hidden: "labels", "status" (the
//                       labels and the status information bar) or "all"
//                       (those and the Plan Position Indicator)
//   &touch=off          with the touch screen off
//
// There is no window underneath in a browser: what shows through is the page.
// Under a tile there is a stand-in for one, so that the tiles can be seen.

import { pinFrame } from "./access-preview";
import { formFrame, typedFrame } from "./entry-preview";
import type { Alarm, Frame, LabelState, Page, Plotted, Threat, Tile } from "./frame";

const COMMON = ["Up", "Alarms", "Threats", "Ack", "Up", "Down", "Labels", "Enter"];

const ALARMS: Alarm[] = [
  { category: "warning", text: "W – Engine Oil Low Pressure", time: "041230Z", acknowledged: false, state: "active" },
  { category: "warning", text: "W – Automatic Fire Suppression System Crew Compartment Discharged", time: "041212Z", acknowledged: true, state: "active" },
  { category: "caution", text: "C – Fuel Low Level", time: "041228Z", acknowledged: false, state: "active" },
  { category: "caution", text: "C – Battery 2 Low Voltage", time: "041150Z", acknowledged: true, state: "overridden" },
  { category: "caution", text: "C – Coolant High Temperature", time: "041141Z", acknowledged: true, state: "resolved" },
  { category: "advisory", text: "A – Engine Service Due", time: "040800Z", acknowledged: false, state: "active" },
];

// A page's worth of rows (ROWS_PER_PAGE in hmi-core), each with more to say
// than a line holds.
const FULL: Alarm[] = Array.from({ length: 10 }, (_, i) => ({
  ...ALARMS[i % ALARMS.length],
  text: `${ALARMS[i % ALARMS.length].text.slice(0, 1)} – Automatic Fire Suppression System Bottle ${i + 1} Discharge Circuit Open`,
}));

// Threats, with what the picture needs of each as well as what the table does.
const THREATS: (Threat & Plotted)[] = [
  { id: "t1", text: "T – Missile Anti-Tank Guided", time: "041229Z", bearingMils: 5600, rangeM: 1800, state: "current", symbol: "SHAPWMSS-------", label: "Missile", picked: false },
  { id: "t2", text: "T – Laser Rangefinder", time: "041230Z", bearingMils: 800, rangeM: 1200, state: "current", symbol: "SHGPEXL--------", label: "Laser", picked: true },
  { id: "t3", text: "T – Laser Designator", time: "041226Z", bearingMils: 3300, rangeM: null, state: "current", symbol: "SHGPEXL--------", label: "Laser", picked: false },
  { id: "t4", text: "T – Gunshot Small Arms", time: "041228Z", bearingMils: 4700, rangeM: 250, state: "current", symbol: "SHGPEWRR-------", label: "Gunshot", picked: false },
  { id: "t5", text: "T – Gunshot Heavy Machine Gun", time: "041221Z", bearingMils: 1900, rangeM: 900, state: "past", symbol: "SHGPEWRH-------", label: "Gunshot", picked: false },
  { id: "t6", text: "T – Radar Ground Surveillance", time: "041215Z", bearingMils: 2500, rangeM: 1500, state: "current", symbol: "nonsense", label: "Radar", picked: false },
];

// An area of tiles: six windows, of which four can be shown at once. These
// are the ones picked, in the order they were; "SAC-5" is not open.
const WINDOWS = ["SAC-1", "SAC-2", "SAC-3", "SAC-4", "SAC-5", "SAC-6"];
const PICKED = ["SAC-1", "SAC-2", "SAC-4", "SAC-6"];
// Where hmi-core puts one, two, three and four tiles - left, top, width,
// height - with nothing hidden, with the labels hidden, and with the status
// information bar hidden as well: its tests have the same figures (tiles.rs).
type Box = [number, number, number, number];
const TILES: Record<"none" | "labels" | "status", Box[][]> = {
  none: [
    [],
    [[124, 89, 776, 582]],
    [[124, 236, 384, 288], [516, 236, 384, 288]],
    [[124, 88, 384, 288], [516, 88, 384, 288], [320, 384, 384, 288]],
    [[124, 88, 384, 288], [516, 88, 384, 288], [124, 384, 384, 288], [516, 384, 384, 288]],
  ],
  labels: [
    [],
    [[62, 64, 899, 674]],
    [[0, 210, 508, 381], [516, 210, 508, 381]],
    [[64, 64, 444, 333], [516, 64, 444, 333], [290, 405, 444, 333]],
    [[64, 64, 444, 333], [516, 64, 444, 333], [64, 405, 444, 333], [516, 405, 444, 333]],
  ],
  status: [
    [],
    [[40, 30, 944, 708]],
    [[0, 193, 508, 381], [516, 193, 508, 381]],
    [[41, 30, 467, 350], [516, 30, 467, 350], [278, 388, 467, 350]],
    [[41, 30, 467, 350], [516, 30, 467, 350], [41, 388, 467, 350], [516, 388, 467, 350]],
  ],
};

/** Stands in for the display server: puts under each tile, where a window
 *  would be, something to be seen through it. */
function standIns(tiles: Tile[]): void {
  const under = document.createElement("div");
  under.className = "display";
  const fit = () => (under.style.transform = `scale(${Math.min(innerWidth / 1024, innerHeight / 768)})`);
  fit();
  addEventListener("resize", fit);
  for (const tile of tiles) {
    const standIn = document.createElement("div");
    standIn.textContent = `${tile.label}\n${tile.width} x ${tile.height}`;
    Object.assign(standIn.style, {
      position: "absolute",
      left: `${tile.x}px`,
      top: `${tile.y}px`,
      width: `${tile.width}px`,
      height: `${tile.height}px`,
      display: "grid",
      placeItems: "center",
      textAlign: "center",
      whiteSpace: "pre",
      fontSize: "28px",
      background: "repeating-linear-gradient(45deg, rgb(70, 80, 90) 0 16px, rgb(84, 96, 108) 16px 32px)",
    });
    under.append(standIn);
  }
  document.body.prepend(under);
}

export function previewFrame(): Frame {
  const asked = new URLSearchParams(location.search);
  const external = asked.get("external");
  const sa = asked.get("sa");
  const picked = PICKED.slice(0, Math.max(0, Math.min(PICKED.length, Number(sa) || 0)));
  const alarms = asked.get("alarms");
  const threats = asked.get("threats");
  const message = asked.get("message");
  const threat = asked.get("threat");
  const detail = alarms === "detail" || threats === "detail";
  const plotted = threats === "plot";
  const hidden = asked.get("hidden");
  const touch = asked.get("touch") !== "off";

  const side = (text: string, state: LabelState, n: number) => ({ key: `F${n}`, text, state });
  const sides: [string, LabelState][] =
    threats !== null
      ? [
          ["Sort By\nType", detail || plotted ? "unavailable" : "selected"],
          ["Sort By\nTime", detail || plotted ? "unavailable" : "available"],
          ["Hide\nPast", detail ? "unavailable" : "available"],
          ["Detail", detail ? "unavailable" : "available"],
          ["Page Up", "unavailable"],
          ["Page Down", threats === "full" ? "available" : "unavailable"],
          ["", "blank"],
          ["", "blank"],
          ["Cancel\nSelected", detail ? "unavailable" : "available"],
          ["Cancel All", detail ? "unavailable" : "available"],
          ["", "blank"],
          [plotted ? "View\nTable" : "View\nPPI", detail ? "unavailable" : "available"],
        ]
      : alarms === null
      ? sa === null
        ? // The SYS page: the touch screen, off and on.
          Array.from({ length: 12 }, (_, i): [string, LabelState] =>
            i === 11 && external === null ? [touch ? "Touch\nOff" : "Touch\nOn", "available"] : ["", "blank"],
          )
        : // A window on each button: shown, or to be had, or not - it is not
          // open, or there are as many shown as there are tiles.
          Array.from({ length: 12 }, (_, i): [string, LabelState] =>
            i >= WINDOWS.length
              ? ["", "blank"]
              : [
                  WINDOWS[i],
                  picked.includes(WINDOWS[i])
                    ? "selected"
                    : WINDOWS[i] === "SAC-5" || picked.length === PICKED.length
                      ? "unavailable"
                      : "available",
                ],
          )
      : [
          ["Sort By\nCategory", detail ? "unavailable" : "selected"],
          ["Sort By\nTime", detail ? "unavailable" : "available"],
          ["Hide\nAcked", detail ? "unavailable" : "available"],
          ["Detail", detail ? "unavailable" : "available"],
          ["Page Up", "unavailable"],
          ["Page Down", "unavailable"],
          ["Ack\nSelected", "unavailable"],
          ["Ack All", detail ? "unavailable" : "available"],
          ["Clear\nSelected", detail ? "unavailable" : "available"],
          ["Clear All", detail ? "unavailable" : "available"],
          ["Override\nSelected", detail ? "unavailable" : "available"],
          ["", "blank"],
        ];

  const row = ({ id, text, time, bearingMils, rangeM, state, picked }: Threat & Plotted) => ({ id, text, time, bearingMils, rangeM, state, selected: picked });
  const threatsPage = (): Page => ({
    kind: "threats",
    counts: { current: 5, past: 1 },
    sensors: [
      { name: "Laser Warning", status: "ok" },
      { name: "Shot Detection", status: "degraded" },
      { name: "Missile Warning", status: "failed" },
      { name: "Radar Warning", status: "off" },
    ],
    view: plotted
      ? {
          kind: "plot",
          rangeM: 2000,
          threats: THREATS.map(({ id, symbol, label, bearingMils, rangeM, state, picked }) => ({ id, symbol, label, bearingMils: bearingMils!, rangeM, state, picked })),
          elsewhere: 1,
        }
      : {
          kind: "table",
          page: 1,
          pages: threats === "full" ? 2 : 1,
          rows:
            threats === "full"
              ? Array.from({ length: 8 }, (_, i) => ({ ...row(THREATS[i % THREATS.length]), id: `t${i}`, text: `T – Missile Anti-Tank Guided Semi-Automatic Command To Line Of Sight ${i + 1}` }))
              : THREATS.map(row),
        },
    detail: detail
      ? { ...row(THREATS[1]), time: "041230Z OCT 24", type: "Laser", description: "Rangefinder", priority: "high", acknowledged: false, detail: "1064 nm, pulsed.\nFront left sensor." }
      : null,
  });

  const page: Page =
    threats !== null
      ? threatsPage()
      : alarms !== null
      ? {
          kind: "alarms",
          unacknowledged: { warnings: 1, cautions: 1, advisories: 1 },
          acknowledged: { warnings: 1, cautions: 2, advisories: 0 },
          page: 1,
          pages: alarms === "full" ? 2 : 1,
          rows: (alarms === "full" ? FULL : ALARMS).map((alarm, i) => ({ ...alarm, id: `a${i}`, selected: i === 1 })),
          detail: detail
            ? { ...ALARMS[1], time: "041212Z OCT 24", detail: "Bottle 2 discharged at 041212Z.\nBottle 1 remains." }
            : null,
        }
      : sa !== null
        ? {
            kind: "tiles",
            tiles: TILES[hidden === null ? "none" : hidden === "labels" ? "labels" : "status"][picked.length].map(
              ([x, y, width, height], i) => ({ label: picked[i], x, y, width, height }),
            ),
          }
        : external !== null
        ? { kind: "external", overlay: external || null, keyboard: null }
        : {
            kind: "own",
            title: "System",
            rows: [
              { name: "HMI version", value: "preview" },
              { name: "Kernel", value: "n/a" },
              { name: "HMI started", value: "n/a" },
              { name: "First frame", value: "n/a" },
              { name: "Touch screen", value: touch ? "On" : "Off" },
              { name: "Last button", value: "none" },
            ],
          };

  if (page.kind === "tiles") standIns(page.tiles);

  const frame: Frame = {
    areas: ["SA", "WPN", "DEF", "SYS", "DRV", "STR", "COM", "BMS"].map((id) => ({
      id,
      state:
        id === "SA" ? (sa === null ? "absent" : "selected")
        : id === "SYS" ? (external === null && sa === null ? "selected" : "available")
        : id === "WPN" && external !== null ? (sa === null ? "selected" : "available")
        : id === "BMS" && external !== null ? "unavailable"
        : "absent",
    })),
    status: {
      dtg: "041230Z OCT 24",
      position: null,
      headingMils: null,
      alarms: alarms !== null || message !== null ? { warnings: 2, cautions: 1, advisories: 1 } : { warnings: 0, cautions: 0, advisories: 0 },
    },
    labels: [
      ...sides.map(([text, state], i) => side(text, state, i + 1)),
      ...COMMON.map((text, i) => ({
        key: `F${13 + i}`,
        text: i === 0 && detail ? "Cancel" : text,
        state: ((i === 1 && alarms !== null) || (i === 2 && threats !== null) ? "selected" : "available") as LabelState,
      })),
    ],
    alarm:
      message === "caution"
        ? { category: "caution", text: "C – Fuel Low Level", acknowledged: false }
        : message !== null
          ? { category: "warning", text: message === "long" ? ALARMS[1].text : ALARMS[0].text, acknowledged: message === "acknowledged" }
          : null,
    threat: threat === null ? null : { text: THREATS[1].text, acknowledged: threat === "acknowledged" },
    // Where the frame has the Plan Position Indicator: on the HMI's own area
    // pages, on a page of tiles and beside the threats table, and over
    // another program's picture while there is a threat to mark.
    ppi:
      hidden === "all" || alarms !== null || plotted || (external !== null && sa === null && threat === null)
        ? null
        : {
            threats:
              threat === null && threats === null
                ? []
                : THREATS.filter((t) => t.state === "current").map((t) => ({ bearingMils: t.bearingMils!, picked: threats !== null && t.picked })),
          },
    page,
    hidden: { labels: hidden !== null, status: hidden === "status" || hidden === "all" },
    touch,
  };
  // A form's page is STR's, and has the rest of the frame as it is here.
  const form = asked.get("form");
  if (form !== null) return formFrame(frame, form);
  // Or a PIN is being asked for, over whatever it was.
  const pin = asked.get("pin");
  if (pin !== null) return pinFrame(frame, pin);
  // And the on-screen keyboard is up over a program's that can be typed into.
  const keyboard = asked.get("keyboard");
  return keyboard === null ? frame : typedFrame(frame, keyboard);
}
