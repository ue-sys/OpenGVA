// milsymbol's ES module entry, which has the library's symbol sets apart so
// that one can be taken without the others. Its package.json does not list
// it (vite.config.ts says where it is) or describe it: `ms` is what the main
// entry is, before any symbols have been added to it.
declare module "milsymbol/index.mjs" {
  import type * as milsymbol from "milsymbol";

  export const ms: typeof milsymbol & { addIcons(icons: object[]): void };
  /** The symbols of MIL-STD-2525C. */
  export const std2525c: object[];
}
