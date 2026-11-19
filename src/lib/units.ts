import { writable, get } from "svelte/store";

export type TempUnit = "c" | "f";

/** The unit the temperatures are written in, as the settings say. The
 * readings always arrive in degrees Celsius; only what is shown changes. */
export const tempUnit = writable<TempUnit>("c");

export function setTempUnit(unit: string | null | undefined) {
  tempUnit.set(unit === "f" ? "f" : "c");
}

/** A temperature in degrees Celsius, written in the unit chosen. */
export function formatTemp(celsius: number | null | undefined, unit: TempUnit = get(tempUnit)): string {
  if (celsius == null) return "—";
  return unit === "f" ? `${Math.round((celsius * 9) / 5 + 32)} °F` : `${Math.round(celsius)} °C`;
}
