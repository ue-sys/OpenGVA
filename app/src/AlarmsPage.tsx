import { For, Show } from "solid-js";
import type { Alarm, AlarmCounts, Page } from "./frame";
import { touch } from "./touch";

type AlarmsPageData = Extract<Page, { kind: "alarms" }>;

// A category by name, as the table and the detail give it: colour repeats
// what the word says and is never the only thing saying it.
const CATEGORY = { warning: "Warning", caution: "Caution", advisory: "Advisory" };
const STATE = { active: "Active", overridden: "Overridden", resolved: "Resolved" };

/** The Alarms page: how many alarms have and have not been
 *  acknowledged, which page of the table this is, and the table. */
export function AlarmsPage(props: { page: AlarmsPageData }) {
  return (
    <div class="page menu alarms-page">
      <header>
        <h1>Alarms</h1>
        <span>
          Page {props.page.page} of {props.page.pages}
        </span>
      </header>

      <div class="menu-counts">
        <Counts name="Unacknowledged" counts={props.page.unacknowledged} />
        <Counts name="Acknowledged" counts={props.page.acknowledged} />
      </div>

      {/* A row to an alarm: what it is, when, of what category
          and whether acknowledged - and whether it is still the case. A row
          touched is the row picked. */}
      <table class="menu-table">
        <thead>
          <tr>
            <th>Alarm</th>
            <th class="time">Time</th>
            <th class="category">Category</th>
            <th class="acknowledged">Ack</th>
            <th class="state">State</th>
          </tr>
        </thead>
        <tbody>
          <For
            each={props.page.rows}
            fallback={
              <tr class="none">
                <td colSpan={5}>No alarms</td>
              </tr>
            }
          >
            {(row) => (
              <tr
                class={`${row.category} ${row.state}${row.selected ? " selected" : ""}`}
                onPointerDown={(event) => touch(event, { kind: "row", id: row.id })}
              >
                <td>
                  <div class="text">{row.text}</div>
                </td>
                <td class="time">{row.time}</td>
                <td class="category">{CATEGORY[row.category]}</td>
                <td class="acknowledged">{row.acknowledged ? "Yes" : "No"}</td>
                <td class="state">{STATE[row.state]}</td>
              </tr>
            )}
          </For>
        </tbody>
      </table>

      <Show when={props.page.detail}>{(alarm) => <Detail alarm={alarm()} />}</Show>
    </div>
  );
}

function Counts(props: { name: string; counts: AlarmCounts }) {
  return (
    <span>
      <span class="name">{props.name}</span>
      <span class="count warning">W{props.counts.warnings}</span>
      <span class="count caution">C{props.counts.cautions}</span>
      <span class="count advisory">A{props.counts.advisories}</span>
    </span>
  );
}

/** One alarm at length. */
function Detail(props: { alarm: Alarm & { detail: string | null } }) {
  return (
    <div class={`menu-detail ${props.alarm.category}`}>
      <h2>{props.alarm.text}</h2>
      <dl>
        <dt>Category</dt>
        <dd class="category">{CATEGORY[props.alarm.category]}</dd>
        <dt>Time</dt>
        <dd>{props.alarm.time}</dd>
        <dt>Acknowledged</dt>
        <dd>{props.alarm.acknowledged ? "Yes" : "No"}</dd>
        <dt>State</dt>
        <dd>{STATE[props.alarm.state]}</dd>
      </dl>
      <p>{props.alarm.detail ?? "No further detail."}</p>
    </div>
  );
}
