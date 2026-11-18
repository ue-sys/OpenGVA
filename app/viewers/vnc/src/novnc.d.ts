// noVNC's one entry is its RFB class. The typings there are for it
// (@types/novnc__novnc) describe it at the path it had when noVNC was
// published as CommonJS, which the package no longer offers: this says that
// what is at the entry now is what they describe.
declare module "@novnc/novnc" {
  export { default } from "@novnc/novnc/lib/rfb";
}
