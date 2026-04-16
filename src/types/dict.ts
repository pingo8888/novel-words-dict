export type NameType =
  | "both"
  | "surname"
  | "given"
  | "place"
  | "gear"
  | "item"
  | "skill"
  | "faction"
  | "nickname"
  | "creature"
  | "others";

export type GenderType = "both" | "male" | "female";
export type GenreType = "east" | "west";
export type ToastTone = "info" | "error";

export type NameTypeFilter = "all" | NameType;
export type GenderTypeFilter = "all" | GenderType;
export type GenreTypeFilter = "all" | GenreType;
