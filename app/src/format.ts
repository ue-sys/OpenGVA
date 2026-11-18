// How the HMI writes quantities, wherever it writes them.

/** An angle in NATO mils: four figures, as a heading is given. */
export const mils = (angle: number) => `${String(angle).padStart(4, "0")} mils`;

/** A distance in metres. */
export const metres = (distance: number) => `${distance} m`;

/** Mils as the degrees a drawing is turned by: 6400 to the circle. */
export const degrees = (angle: number) => (angle * 360) / 6400;
