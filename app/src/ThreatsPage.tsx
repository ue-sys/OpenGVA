import { For, Show } from "solid-js";
import { metres, mils } from "./format";
import type { Page } from "./frame";
import { ThreatPlot } from "./ThreatPlot";
import { touch } from "./touch";

type ThreatsPageData = Extract<Page, { kind: "threats" }>;
type Detail = NonNullable<ThreatsPageData["detail"]>;

// By name, as the page gives them: colour repeats what the word says and is
// never the only thing saying it.
const STATE = { current: "Current", past: "Past" };
const PRIORITY = { high: "High", medium: "Medium", low: "Low" };
const STATUS = { ok: "OK", degraded: "Degraded", failed: "Failed", off: "Off" };

/** The Threats page: the threats as a table, with how
 *  many there are and how the sensors that look for them are -
 *  or, every one of them at once, as a picture. */
export function ThreatsPage(props: { page: ThreatsPageData; headingMils: number | null }) {
  const table = () => (props.page.view.kind === "table" ? props.page.view : undefined);
  const plot = () => (props.page.view.kind === "plot" ? props.page.view : undefined);

  return (
    <div class="page menu threats-page" classList={{ plotted: plot() !== undefined }}>
      {/* The table has the Plan Position Indicator in its usual corner, and
          what the page has to say of itself goes beside that. */}
      <Show when={table()}>
        {(view) => (
          <>
            <div class="beside-ppi">
              <header>
                <h1>Threats</h1>
                <span>
                  Page {view().page} of {view().pages}
                </span>
              </header>
              <Counts counts={props.page.counts} />
              <Sensors sensors={props.page.sensors} />
            </div>

            {/* A row to a threat: what it is, when it was
                detected, where it is and whether it is still there. A row
                touched is the row picked. */}
            <table class="menu-table">
              <thead>
                <tr>
                  <th>Threat</th>
                  <th class="time">Time</th>
                  <th class="bearing">Rel Bearing</th>
                  <th class="range">Range</th>
                  <th class="standing">Status</th>
                </tr>
              </thead>
              <tbody>
                <For
                  each={view().rows}
                  fallback={
                    <tr class="none">
                      <td colSpan={5}>No threats</td>
                    </tr>
                  }
                >
                  {(row) => (
                    <tr
                      class={`${row.state}${row.selected ? " selected" : ""}`}
                      onPointerDown={(event) => touch(event, { kind: "row", id: row.id })}
                    >
                      <td>
                        <div class="text">{row.text}</div>
                      </td>
                      <td class="time">{row.time}</td>
                      <td class="bearing">{row.bearingMils === null ? "–" : mils(row.bearingMils)}</td>
                      <td class="range">{row.rangeM === null ? "–" : metres(row.rangeM)}</td>
                      <td class="standing">{STATE[row.state]}</td>
                    </tr>
                  )}
                </For>
              </tbody>
            </table>
          </>
        )}
      </Show>

      {/* The picture has the page to itself. What it cannot show - a threat
          with no bearing has no place on it - it says that it is not showing. */}
      <Show when={plot()}>
        {(view) => (
          <>
            <ThreatPlot headingMils={props.headingMils} rangeM={view().rangeM} threats={view().threats} />
            <Show when={view().elsewhere > 0}>
              <div class="elsewhere">Bearing not known: {view().elsewhere}</div>
            </Show>
          </>
        )}
      </Show>

      <Show when={props.page.detail}>{(threat) => <ThreatDetail threat={threat()} />}</Show>
    </div>
  );
}

function Counts(props: { counts: ThreatsPageData["counts"] }) {
  return (
    <div class="menu-counts">
      <span>
        <span class="name">Current</span>
        {props.counts.current}
      </span>
      <span>
        <span class="name">Past</span>
        {props.counts.past}
      </span>
    </div>
  );
}

/** The sensors that look for threats, and how each says it is: the crew are
 *  to know what the picture is made from, and that with none it is no
 *  picture. */
function Sensors(props: { sensors: ThreatsPageData["sensors"] }) {
  return (
    <div class="sensors">
      <span class="name">Sensors</span>
      <For each={props.sensors} fallback={<span class="sensor none">None reporting</span>}>
        {(sensor) => (
          <span class={`sensor ${sensor.status}`}>
            {sensor.name} <span class="how">{STATUS[sensor.status]}</span>
          </span>
        )}
      </For>
    </div>
  );
}

/** One threat at length. */
function ThreatDetail(props: { threat: Detail }) {
  return (
    <div class={`menu-detail ${props.threat.state}`}>
      <h2>{props.threat.text}</h2>
      <dl>
        <dt>Type</dt>
        <dd>{props.threat.type}</dd>
        <Show when={props.threat.description}>
          <dt>Description</dt>
          <dd>{props.threat.description}</dd>
        </Show>
        <dt>Priority</dt>
        <dd>{PRIORITY[props.threat.priority]}</dd>
        <dt>Time</dt>
        <dd>{props.threat.time}</dd>
        <dt>Bearing</dt>
        <dd>
          {props.threat.bearingMils === null ? "Not known" : `${mils(props.threat.bearingMils)} from the vehicle's front`}
        </dd>
        <dt>Range</dt>
        <dd>{props.threat.rangeM === null ? "Not known" : metres(props.threat.rangeM)}</dd>
        <dt>Status</dt>
        <dd>{STATE[props.threat.state]}</dd>
        <dt>Acknowledged</dt>
        <dd>{props.threat.acknowledged ? "Yes" : "No"}</dd>
      </dl>
      <p>{props.threat.detail ?? "No further detail."}</p>
    </div>
  );
}
