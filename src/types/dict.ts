export type NameType =
  | "both"
  | "surname"
  | "given"
  | "place"
  | "monster"
  | "gear"
  | "food"
  | "item"
  | "skill"
  | "faction"
  | "title"
  | "nickname"
  | "creature"
  | "others";

export type GenderType = "both" | "male" | "female";
export type GenreType = "east" | "west";
export type ToastTone = "info" | "error";

export type NameTypeFilter = "all" | NameType;
export type GenderTypeFilter = "all" | GenderType;
export type GenreTypeFilter = "all" | GenreType;

export interface NameEntry {
  term: string;
  group: string;
  nameType: NameType;
  genderType: GenderType;
  genre: GenreType;
}

export interface QueryNameEntry extends NameEntry {
  dictId: string;
  dictName: string;
  editable: boolean;
}
