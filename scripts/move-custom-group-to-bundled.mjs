import fs from "node:fs";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";

function printHelp() {
  console.log(`Usage:
  node scripts/move-custom-group-to-bundled.mjs <fileName> <dictId> <group> [options]

Arguments:
  fileName              Target bundled dict file name under ./dict (must end with .json)
  dictId                Dict id to initialize when target file has no dictId
  group                 Group value to move from custom entries (trim exact match)

Options:
  --dry-run             Preview changes without writing files
  --settings <path>     Settings file path (default: %APPDATA%/com.local.novel-words-dict/settings.json)
  --custom-db <path>    Custom SQLite database path (preferred)
  --custom <path>       Custom entries.json path (overrides --settings)
  --dict-dir <path>     Bundled dict directory (default: ./dict)
  --backup              Create backup before writing changed files
  --help                Show this message
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
  const positionals = [];

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
      options.settingsPath = argv[i + 1] ?? null;
      i += 1;
      continue;
    }
    if (arg === "--custom-db") {
      options.customDbPath = argv[i + 1] ?? null;
      i += 1;
      continue;
    }
    if (arg === "--custom") {
      options.customPath = argv[i + 1] ?? null;
      i += 1;
      continue;
    }
    if (arg === "--dict-dir") {
      options.dictDir = path.resolve(argv[i + 1] ?? "");
      i += 1;
      continue;
    }
    if (arg.startsWith("--")) {
      throw new Error(`Unknown argument: ${arg}`);
    }
    positionals.push(arg);
  }

  if (options.help) {
    return { options, params: null };
  }
  if (positionals.length !== 3) {
    throw new Error("Expected 3 arguments: <fileName> <dictId> <group>");
  }

  const fileName = validateFileName(positionals[0]);
  const dictId = sanitizeDictId(positionals[1]);
  if (!dictId) {
    throw new Error("dictId is invalid; use letters/numbers/-/_");
  }
  const group = positionals[2].trim();

  if (options.settingsPath) {
    options.settingsPath = path.resolve(options.settingsPath);
  }
  if (options.customDbPath) {
    options.customDbPath = path.resolve(options.customDbPath);
  }
  if (options.customPath) {
    options.customPath = path.resolve(options.customPath);
  }

  return {
    options,
    params: { fileName, dictId, group },
  };
}

function validateFileName(input) {
  const trimmed = input.trim();
  if (!trimmed) {
    throw new Error("fileName is required");
  }
  if (trimmed !== path.basename(trimmed)) {
    throw new Error("fileName must be a file name only (no directory path)");
  }
  if (!trimmed.toLowerCase().endsWith(".json")) {
    throw new Error("fileName must end with .json");
  }
  return trimmed;
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

function normalizeTermKey(term) {
  return term.trim().toLowerCase();
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
        const group = normalizeGroup(entry.group);
        insert.run(
          normalizeTermKey(term),
          term,
          group,
          typeof entry.nameType === "string" ? entry.nameType.trim().toLowerCase() : "both",
          typeof entry.genderType === "string" ? entry.genderType.trim().toLowerCase() : "both",
          typeof entry.genre === "string" ? entry.genre.trim().toLowerCase() : "west",
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

function isEntryItem(item) {
  return (
    item !== null &&
    typeof item === "object" &&
    Object.prototype.hasOwnProperty.call(item, "term") &&
    typeof item.term === "string"
  );
}

function normalizeTerm(term) {
  return term.trim().toLowerCase();
}

function normalizeGroup(group) {
  if (typeof group !== "string") {
    return "";
  }
  return group.trim();
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

function dedupeEntries(entries) {
  const unique = [];
  const seen = new Set();
  let duplicateCount = 0;

  for (const entry of entries) {
    const term = typeof entry.term === "string" ? entry.term.trim() : "";
    if (!term) {
      continue;
    }
    const key = normalizeTerm(term);
    if (seen.has(key)) {
      duplicateCount += 1;
      continue;
    }
    seen.add(key);
    unique.push({
      ...entry,
      term,
      group: normalizeGroup(entry.group),
    });
  }

  return { unique, duplicateCount };
}

function ensureTargetFileData(targetPath, dictId) {
  if (!fs.existsSync(targetPath)) {
    return {
      existed: false,
      metaItems: [{ dictId }],
      entries: [],
      rawContent: "",
    };
  }

  const rawContent = fs.readFileSync(targetPath, "utf8");
  const parsed = readJsonArray(targetPath);
  const metaItems = [];
  const entries = [];
  let hasMetaDictId = false;

  for (const item of parsed) {
    if (isEntryItem(item)) {
      entries.push(item);
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

  return {
    existed: true,
    metaItems,
    entries,
    rawContent,
  };
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
    const { options, params } = parseArgs(process.argv.slice(2));
    if (options.help) {
      printHelp();
      return;
    }

    if (!params) {
      throw new Error("Arguments are required");
    }
    if (!fs.existsSync(options.dictDir)) {
      throw new Error(`Bundled dict directory not found: ${options.dictDir}`);
    }

    const customSource = resolveCustomSource(options);
    const customArray = readCustomEntries(customSource);
    const targetPath = path.join(options.dictDir, params.fileName);

    const keptCustomItems = [];
    const movedEntries = [];
    let movedCount = 0;

    for (const item of customArray) {
      if (!isEntryItem(item)) {
        keptCustomItems.push(item);
        continue;
      }

      const term = item.term.trim();
      const group = normalizeGroup(item.group);
      if (!term || group !== params.group) {
        keptCustomItems.push(item);
        continue;
      }

      movedEntries.push({
        ...item,
        term,
        group,
      });
      movedCount += 1;
    }

    const targetData = ensureTargetFileData(targetPath, params.dictId);
    const deduped = dedupeEntries([...targetData.entries, ...movedEntries]);
    deduped.unique.sort(compareByGroupThenTerm);

    const nextTargetContent = formatJsonArray([...targetData.metaItems, ...deduped.unique]);
    const targetChanged = !targetData.existed || nextTargetContent !== targetData.rawContent;

    const customChanged = movedCount > 0;

    const backupFiles = new Set();
    const backupStamp = new Date().toISOString().replaceAll(":", "").replaceAll(".", "");

    if (!options.dryRun) {
      if (targetChanged) {
        backupFileIfNeeded(targetPath, options.backup, backupStamp, backupFiles);
        fs.writeFileSync(targetPath, nextTargetContent, "utf8");
      }
      if (customChanged) {
        backupFileIfNeeded(customSource.path, options.backup, backupStamp, backupFiles);
        writeCustomEntries(customSource, keptCustomItems);
      }
    }

    console.log(`custom source: ${customSource.path} (${customSource.kind})`);
    console.log(`target file: ${targetPath}`);
    console.log(`dictId arg: ${params.dictId}`);
    console.log(`group arg: ${params.group || "(empty)"}`);
    console.log(`moved from custom: ${movedCount}`);
    console.log(`duplicates removed in target: ${deduped.duplicateCount}`);
    console.log(`target file: ${targetChanged ? "updated" : "unchanged"}`);
    console.log(`custom entries: ${customChanged ? "updated" : "unchanged"}`);
    if (options.backup && backupFiles.size > 0) {
      console.log(`backup files: ${backupFiles.size}`);
    }
    if (options.dryRun) {
      console.log("dry run mode: no files were changed.");
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}

main();

