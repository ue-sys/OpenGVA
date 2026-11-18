// The page: a VNC server's screen, or a line that says it is not there.
//
// The VNC is noVNC's. Its RFB object connects, draws the server's screen
// into the page and sends the server what is done to it: a mouse's
// movements, and touches as the mouse movements a VNC server understands.
// None of noVNC's own user interface is used. What is here is what noVNC
// leaves to whoever uses it: which server, saying so when it is not there,
// and trying again.

import RFB from "@novnc/novnc";
import { reportConnected, reportDisconnected, theView } from "./bridge";
import "./viewer.css";

// A server that is not there is tried again a second later, and then after
// twice as long each time, up to ten seconds: one that is starting is found
// soon after it is up, and one that is switched off is not called on many
// times a second for as long as it stays off.
const SOONEST = 1000;
const LATEST = 10000;

const picture = document.getElementById("picture")!;
const status = document.getElementById("status")!;

/** Says `what` where the picture would be, or with `null` nothing. */
function say(what: string | null): void {
  status.textContent = what;
  status.hidden = what === null;
}

// Nothing of the browser's comes up on the window, whatever is done to it.
window.addEventListener("contextmenu", (event) => event.preventDefault());

const view = await theView();
let wait = SOONEST;
// What the window says of a server whose screen is not to be had - a
// sentence, not a headline. That there is none, until it has
// been had once; from then on, that it has gone.
let missing = `No VNC server at ${view.server}`;

/** Tries for the server's screen; and when that has ended, however it has,
 *  says so and tries again. */
function connect(): void {
  let connected = false;
  // What the window is to say when this try has ended, and why it ended, in
  // the words of whichever end said so, for the log.
  let saying = missing;
  let why = "";

  const ended = () => {
    say(saying);
    reportDisconnected(why || (connected ? "the connection was lost" : "noVNC could not speak to it"));
    setTimeout(connect, wait);
    wait = Math.min(wait * 2, LATEST);
  };

  let rfb: RFB;
  try {
    // The socket is made here, not by noVNC, to hear why it is closed: the
    // bridge says, and noVNC does not pass it on. This is the first to
    // listen, so it has heard by the time noVNC says the connection is gone.
    const socket = new WebSocket(view.url);
    let opened = false;
    socket.addEventListener("open", () => (opened = true));
    socket.addEventListener("close", (event) => {
      why ||= event.reason || (opened ? "" : "the page could not reach the bridge");
    });
    // (The typings would have a user's name with every password. noVNC
    // would not, and nor do most servers.)
    const credentials = view.password === null ? {} : { password: view.password };
    rfb = new RFB(picture, socket, { credentials } as ConstructorParameters<typeof RFB>[2]);
  } catch (error) {
    why = String(error);
    ended();
    return;
  }

  rfb.viewOnly = view.viewOnly;
  rfb.scaleViewport = view.scale === "fit";
  // Not scaled, a screen larger than the window is shown as far as it goes
  // in, rather than given scroll bars.
  rfb.clipViewport = view.scale === "none";
  rfb.background = "rgb(0, 0, 0)";
  // A server need not say what its pointer looks like, and with none there
  // would be nothing to see of a mouse over its screen.
  rfb.showDotCursor = !view.viewOnly;

  rfb.addEventListener("connect", () => {
    connected = true;
    wait = SOONEST;
    saying = missing = `The VNC server at ${view.server} has gone`;
    say(null);
    reportConnected();
  });
  // The server wants what this view has not been given, and there is nobody
  // here to ask for it; or it has been given it, and will not have it.
  // Neither is going to be otherwise a second from now.
  rfb.addEventListener("credentialsrequired", (event) => {
    const wanted = event.detail.types.join(" and ");
    saying = `The VNC server at ${view.server} wants a ${wanted}`;
    why = `it wants a ${wanted}`;
    wait = LATEST;
    rfb.disconnect();
  });
  rfb.addEventListener("securityfailure", (event) => {
    saying = `The VNC server at ${view.server} turned this view away`;
    why = `it turned the view away: ${event.detail.reason ?? `status ${event.detail.status}`}`;
    wait = LATEST;
  });
  rfb.addEventListener("disconnect", ended);
}

say(`Waiting for the VNC server at ${view.server}`);
connect();
