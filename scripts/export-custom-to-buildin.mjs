import fs from "node:fs";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";

const NAME_TYPES = new Set([
  "both",
  "surname",
  "given",
  "place",
  "myth",
  "people",
  "creature",
  "monster",
  "gear",
  "food",
  "item",
  "skill",
  "faction",
  "title",
  "nickname",
  "book",
  "others",
]);
const BUILD_IN_DICT_ID = "build-in";

function printHelp() {
  console.log(`Usage:
  node scripts/export-custom-to-buildin.mjs [options]

Options:
  --dry-run             Preview changes without writing files
  --settings <path>     Settings file path (default: %APPDATA%/com.local.novel-words-dict/settings.json)
  --custom-db <path>    Custom SQLite database path (preferred)
  --custom <path>       Custom entries.json path (overrides --settings)
  --dict-dir <path>     Built-in dict directory (default: ./dict)
  --backup              Create backup before writing changed files
  --help                Show this message

Moves custom entries whose group_name/group is empty into genre-nameType JSON files.
`);
}

function parseArgs(argv) {
  const options = {
    dryRun: false,
    backup: false,
    settingsPath: null,
    customDbPath: null,
    customPath: null,
    dictDir: path.resolve(process.cwd(), "dict"),
    help: false,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--dry-run") {
      options.dryRun = true;
      continue;
    }
    if (arg === "--backup") {
      options.backup = true;
      continue;
    }
    if (arg === "--help" || arg === "-h") {
      options.help = true;
      continue;
    }
    if (arg === "--settings") {
      options.settingsPath = path.resolve(argv[i + 1] ?? "");
      i += 1;
      continue;
    }
    if (arg === "--custom-db") {
      options.customDbPath = path.resolve(argv[i + 1] ?? "");
      i += 1;
      continue;
    }
    if (arg === "--custom") {
      options.customPath = path.resolve(argv[i + 1] ?? "");
      i += 1;
      continue;
    }
    if (arg === "--dict-dir") {
      options.dictDir = path.resolve(argv[i + 1] ?? "");
      i += 1;
      continue;
    }
    throw new Error(`Unknown argument: ${arg}`);
  }

  return options;
}

function sanitizeDictId(value) {
  let out = "";
  for (const ch of value.trim()) {
    if (/^[A-Za-z0-9_-]$/.test(ch)) {
      out += ch.toLowerCase();
    }
  }
  return out;
}

function resolveDefaultSettingsPath() {
  const appData = process.env.APPDATA;
  if (!appData) {
    throw new Error("APPDATA is not available; please provide --custom-db or --custom.");
  }
  return path.join(appData, "com.local.novel-words-dict", "settings.json");
}

function resolveDefaultCustomDbPath() {
  const appData = process.env.APPDATA;
  if (!appData) {
    throw new Error("APPDATA is not available; please provide --custom-db or --custom.");
  }
  return path.join(appData, "com.local.novel-words-dict", "custom.db");
}

function resolveCustomSource(options) {
  if (options.customDbPath) {
    return { kind: "db", path: options.customDbPath };
  }
  if (options.customPath) {
    return { kind: "json", path: options.customPath };
  }

  const defaultCustomDbPath = resolveDefaultCustomDbPath();
  if (fs.existsSync(defaultCustomDbPath)) {
    return { kind: "db", path: defaultCustomDbPath };
  }

  const settingsPath = options.settingsPath ?? resolveDefaultSettingsPath();
  if (!fs.existsSync(settingsPath)) {
    throw new Error(`Settings file not found: ${settingsPath}`);
  }
  const raw = fs.readFileSync(settingsPath, "utf8").trim();
  const settings = raw ? JSON.parse(raw) : {};
  const dictDir =
    typeof settings.dictDir === "string" && settings.dictDir.trim().length > 0
      ? settings.dictDir.trim()
      : path.dirname(settingsPath);
  return { kind: "json", path: path.join(dictDir, "entries.json") };
}

function normalizeTermKey(term) {
  return term.trim().toLowerCase();
}

function normalizeGroup(group) {
  return typeof group === "string" ? group.trim() : "";
}

function normalizeNameType(value) {
  const normalized = typeof value === "string" ? value.trim().toLowerCase() : "";
  if (normalized === "items") {
    return "item";
  }
  if (normalized === "skills") {
    return "skill";
  }
  if (normalized === "other" || normalized === "incantation") {
    return "others";
  }
  return NAME_TYPES.has(normalized) ? normalized : "both";
}

function normalizeGenderType(value) {
  const normalized = typeof value === "string" ? value.trim().toLowerCase() : "";
  if (normalized === "male" || normalized === "female" || normalized === "both") {
    return normalized;
  }
  return "both";
}

function normalizeGenre(value) {
  const normalized = typeof value === "string" ? value.trim().toLowerCase() : "";
  if (normalized === "china" || normalized === "east") {
    return "china";
  }
  if (normalized === "japan") {
    return "japan";
  }
  return "west";
}

function ensureCustomDbSchema(db) {
  db.exec(`
    CREATE TABLE IF NOT EXISTS custom_entries (
      term_key TEXT PRIMARY KEY,
      term TEXT NOT NULL,
      group_name TEXT NOT NULL DEFAULT '',
      name_type TEXT NOT NULL,
      gender_type TEXT NOT NULL,
      genre TEXT NOT NULL
    );
  `);
}

function readJsonArray(filePath) {
  if (!fs.existsSync(filePath)) {
    throw new Error(`File not found: ${filePath}`);
  }
  const raw = fs.readFileSync(filePath, "utf8").trim();
  if (!raw) {
    return [];
  }
  const parsed = JSON.parse(raw);
  if (!Array.isArray(parsed)) {
    throw new Error(`Expected JSON array: ${filePath}`);
  }
  return parsed;
}

function readCustomEntries(source) {
  if (source.kind === "json") {
    return readJsonArray(source.path);
  }

  if (!fs.existsSync(source.path)) {
    throw new Error(`Custom db not found: ${source.path}`);
  }
  const db = new DatabaseSync(source.path);
  try {
    ensureCustomDbSchema(db);
    const rows = db
      .prepare(
        `SELECT term, group_name, name_type, gender_type, genre
         FROM custom_entries
         ORDER BY term COLLATE NOCASE ASC`,
      )
      .all();
    return rows.map((row) => ({
      term: String(row.term ?? ""),
      group: String(row.group_name ?? ""),
      nameType: String(row.name_type ?? "both"),
      genderType: String(row.gender_type ?? "both"),
      genre: String(row.genre ?? "west"),
    }));
  } finally {
    db.close();
  }
}

function writeCustomEntries(source, entries) {
  if (source.kind === "json") {
    fs.writeFileSync(source.path, formatJsonArray(entries), "utf8");
    return;
  }

  const db = new DatabaseSync(source.path);
  try {
    ensureCustomDbSchema(db);
    db.exec("BEGIN");
    try {
      db.prepare("DELETE FROM custom_entries").run();
      const insert = db.prepare(
        `INSERT INTO custom_entries (term_key, term, group_name, name_type, gender_type, genre)
         VALUES (?, ?, ?, ?, ?, ?)`,
      );
      for (const entry of entries) {
        const term = typeof entry.term === "string" ? entry.term.trim() : "";
        if (!term) {
          continue;
        }
        insert.run(
          normalizeTermKey(term),
          term,
          normalizeGroup(entry.group),
          normalizeNameType(entry.nameType),
          normalizeGenderType(entry.genderType),
          normalizeGenre(entry.genre),
        );
      }
      db.exec("COMMIT");
    } catch (error) {
      db.exec("ROLLBACK");
      throw error;
    }
  } finally {
    db.close();
  }
}

function isEntryItem(item) {
  return (
    item !== null &&
    typeof item === "object" &&
    Object.prototype.hasOwnProperty.call(item, "term") &&
    typeof item.term === "string"
  );
}

function extractMetaDictId(item) {
  if (!item || typeof item !== "object") {
    return "";
  }
  const value =
    (typeof item.dictId === "string" ? item.dictId : null) ??
    (typeof item.dict_id === "string" ? item.dict_id : null) ??
    "";
  return sanitizeDictId(value);
}

const pinyinCollator = new Intl.Collator("zh-Hans-u-co-pinyin", {
  sensitivity: "base",
  numeric: true,
});

function compareTextByPinyin(left, right) {
  const byPinyin = pinyinCollator.compare(left, right);
  if (byPinyin !== 0) {
    return byPinyin;
  }
  return left.localeCompare(right, "zh-Hans");
}

function compareByGroupThenTerm(left, right) {
  const byGroup = compareTextByPinyin(normalizeGroup(left.group), normalizeGroup(right.group));
  if (byGroup !== 0) {
    return byGroup;
  }
  return compareTextByPinyin(left.term.trim(), right.term.trim());
}

function toBundledEntry(item) {
  const term = typeof item.term === "string" ? item.term.trim() : "";
  return {
    term,
    group: normalizeGroup(item.group),
    nameType: normalizeNameType(item.nameType),
    genderType: normalizeGenderType(item.genderType),
    genre: normalizeGenre(item.genre),
  };
}

function targetFileName(entry) {
  return `${entry.genre}-${entry.nameType}.json`;
}

function ensureTargetFileData(filePath, dictId) {
  if (!fs.existsSync(filePath)) {
    return {
      existed: false,
      metaItems: [{ dictId }],
      entries: [],
      rawContent: "",
    };
  }

  const rawContent = fs.readFileSync(filePath, "utf8");
  const parsed = readJsonArray(filePath);
  const metaItems = [];
  const entries = [];
  let hasMetaDictId = false;

  for (const item of parsed) {
    if (isEntryItem(item)) {
      entries.push(toBundledEntry(item));
      continue;
    }
    if (!hasMetaDictId && extractMetaDictId(item)) {
      hasMetaDictId = true;
    }
    metaItems.push(item);
  }

  if (!hasMetaDictId) {
    metaItems.unshift({ dictId });
  }

  return { existed: true, metaItems, entries, rawContent };
}

function dedupeEntries(entries) {
  const unique = [];
  const seen = new Set();
  let duplicateCount = 0;

  for (const entry of entries) {
    const normalized = toBundledEntry(entry);
    if (!normalized.term) {
      continue;
    }
    const key = normalizeTermKey(normalized.term);
    if (seen.has(key)) {
      duplicateCount += 1;
      continue;
    }
    seen.add(key);
    unique.push(normalized);
  }

  return { unique, duplicateCount };
}

function formatJsonArray(items) {
  if (items.length === 0) {
    return "[]\n";
  }
  return `[\n${items.map((item) => JSON.stringify(item)).join(",\n")}\n]\n`;
}

function backupFileIfNeeded(filePath, backup, backupStamp, backupFiles) {
  if (!backup || backupFiles.has(filePath) || !fs.existsSync(filePath)) {
    return;
  }
  const backupPath = `${filePath}.${backupStamp}.bak`;
  fs.copyFileSync(filePath, backupPath);
  backupFiles.add(filePath);
}

function main() {
  try {
    const options = parseArgs(process.argv.slice(2));
    if (options.help) {
      printHelp();
      return;
    }

    const customSource = resolveCustomSource(options);
    const customArray = readCustomEntries(customSource);
    const movedBuckets = new Map();
    const keptCustomItems = [];
    let skippedNonEmptyGroupCount = 0;
    let skippedInvalidTermCount = 0;
    let movedCount = 0;

    for (const item of customArray) {
      if (!isEntryItem(item)) {
        keptCustomItems.push(item);
        continue;
      }

      const entry = toBundledEntry(item);
      if (!entry.term) {
        skippedInvalidTermCount += 1;
        keptCustomItems.push(item);
        continue;
      }
      if (entry.group) {
        skippedNonEmptyGroupCount += 1;
        keptCustomItems.push(item);
        continue;
      }

      const fileName = targetFileName(entry);
      const bucket = movedBuckets.get(fileName) ?? [];
      bucket.push(entry);
      movedBuckets.set(fileName, bucket);
      movedCount += 1;
    }

    const targetResults = [];
    const changedFiles = [];
    const backupFiles = new Set();
    const backupStamp = new Date().toISOString().replaceAll(":", "").replaceAll(".", "");
    let duplicateSkippedCount = 0;

    for (const [fileName, movedEntries] of [...movedBuckets.entries()].sort()) {
      const filePath = path.join(options.dictDir, fileName);
      const targetData = ensureTargetFileData(filePath, BUILD_IN_DICT_ID);
      const deduped = dedupeEntries([...targetData.entries, ...movedEntries]);
      deduped.unique.sort(compareByGroupThenTerm);
      duplicateSkippedCount += deduped.duplicateCount;

      const nextContent = formatJsonArray([...targetData.metaItems, ...deduped.unique]);
      const changed = !targetData.existed || nextContent !== targetData.rawContent;
      if (changed) {
        changedFiles.push(filePath);
      }
      targetResults.push({
        fileName,
        filePath,
        movedIn: movedEntries.length,
        changed,
        nextContent,
      });
    }

    const customChanged = movedCount > 0;
    if (customChanged) {
      changedFiles.push(customSource.path);
    }

    if (!options.dryRun) {
      fs.mkdirSync(options.dictDir, { recursive: true });
      for (const result of targetResults) {
        if (!result.changed) {
          continue;
        }
        backupFileIfNeeded(result.filePath, options.backup, backupStamp, backupFiles);
        fs.writeFileSync(result.filePath, result.nextContent, "utf8");
      }
      if (customChanged) {
        backupFileIfNeeded(customSource.path, options.backup, backupStamp, backupFiles);
        writeCustomEntries(customSource, keptCustomItems);
      }
    }

    console.log(`custom source: ${customSource.path} (${customSource.kind})`);
    console.log(`built-in dict dir: ${options.dictDir}`);
    console.log(`matched and moved: ${movedCount}`);
    console.log(`non-empty group kept in custom: ${skippedNonEmptyGroupCount}`);
    console.log(`invalid term kept in custom: ${skippedInvalidTermCount}`);
    console.log(`duplicates skipped in target files: ${duplicateSkippedCount}`);
    console.log(`changed files: ${changedFiles.length}`);
    if (options.backup && backupFiles.size > 0) {
      console.log(`backup files: ${backupFiles.size}`);
    }
    for (const result of targetResults) {
      console.log(
        `${result.fileName}: matched ${result.movedIn}, ${result.changed ? "updated" : "unchanged"}`,
      );
    }
    console.log(`custom entries: ${customChanged ? "updated" : "unchanged"}`);
    if (options.dryRun) {
      console.log("dry run mode: no files were changed.");
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}

main();
