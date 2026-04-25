import type { GenderType, NameType } from "../types/dict";
import { isGenderEditableByNameType } from "../utils/nameType";

export function getNameTypeIcons(nameType: NameType): string[] {
  switch (nameType) {
    case "surname":
      return ["姓"];
    case "given":
      return ["名"];
    case "place":
      return ["地"];
    case "myth":
      return ["神"];
    case "people":
      return ["人"];
    case "creature":
      return ["生"];
    case "monster":
      return ["怪"];
    case "gear":
      return ["装"];
    case "food":
      return ["食"];
    case "item":
      return ["物"];
    case "skill":
      return ["技"];
    case "faction":
      return ["势"];
    case "title":
      return ["衔"];
    case "nickname":
      return ["绰"];
    case "book":
      return ["书"];
    case "others":
      return [];
    case "both":
      return ["姓", "名"];
    default:
      if (import.meta.env.DEV) {
        console.warn("Unknown nameType icon mapping:", nameType);
      }
      return [];
  }
}

export function getGenderIconClass(genderType: GenderType): string {
  if (genderType === "male") {
    return "gender-male";
  }
  if (genderType === "female") {
    return "gender-female";
  }
  return "gender-both";
}

export function shouldShowGenderIcon(nameType: NameType): boolean {
  return isGenderEditableByNameType(nameType);
}

export function formatGroupLabel(group: string): string {
  const text = group.trim();
  if (!text) {
    return "〔未分组〕";
  }
  return `〔${text}〕`;
}
