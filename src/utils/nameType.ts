import type { NameType } from "../types/dict";

export function isGenderEditableByNameType(nameType: NameType): boolean {
  return nameType === "surname" || nameType === "given";
}
