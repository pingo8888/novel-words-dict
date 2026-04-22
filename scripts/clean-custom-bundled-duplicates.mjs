import fs from "node:fs";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";

function printHelp() {
  console.log(`Usage:
  node scripts/clean-custom-bundled-duplicates.mjs [options]

Options:
  --dry-run               Preview removals without writing files
  --settings <path>       Settings file path (default: %APPDATA%/com.local.novel-words-dict/settings.json)
  --custom-db <path>      Custom SQLite database path (preferred)
  --custom <path>         Custom entries.json path (overrides --settings)
  --bundled-dir <path>    Bundled dict directory (default: ./dict)
  --backup                Create backup before writing
  --help                  Show this message
`);
}

function parseArgs(argv) {
  const options = {
    dryRun: false,
    backup: false,
    settingsPath: null,
    customDbPath: null,
    customPath: null,
    bundledDir: path.resolve(process.cwd(), "dict"),
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
    if (arg === "--bundled-dir") {
      options.bundledDir = path.resolve(argv[i + 1] ?? "");
      i += 1;
      continue;
    }
    throw new Error(`Unknown argument: ${arg}`);
  }

  if (options.settingsPath) {
    options.settingsPath = path.resolve(options.settingsPath);
  }
  if (options.customDbPath) {
    options.customDbPath = path.resolve(options.customDbPath);
  }
  if (options.customPath) {
    options.customPath = path.resolve(options.customPath);
  }
  return options;
}

function resolveDefaultSettingsPath() {
  const appData = process.env.APPDATA;
  if (!appData) {
    throw new Error("APPDATA is not available; please provide --custom-db or --custom.");
  }
  return path.join(appData, "com.local.novel-words-dict", "settings.json");
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

function normalizeTerm(term) {
  return term.trim().toLowerCase();
}

function isEntryItem(item) {
  return (
    item !== null &&
    typeof item === "object" &&
    Object.prototype.hasOwnProperty.call(item, "term") &&
    typeof item.term === "string"
  );
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
    fs.writeFileSync(source.path, `${JSON.stringify(entries, null, 2)}\n`, "utf8");
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
        if (!isEntryItem(entry)) {
          continue;
        }
        const term = entry.term.trim();
        if (!term) {
          continue;
        }
        const group = typeof entry.group === "string" ? entry.group.trim() : "";
        const nameType =
          typeof entry.nameType === "string" && entry.nameType.trim()
            ? entry.nameType.trim().toLowerCase()
            : "both";
        const genderType =
          typeof entry.genderType === "string" && entry.genderType.trim()
            ? entry.genderType.trim().toLowerCase()
            : "both";
        const genre =
          typeof entry.genre === "string" && entry.genre.trim()
            ? entry.genre.trim().toLowerCase()
            : "west";
        insert.run(normalizeTermKey(term), term, group, nameType, genderType, genre);
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

function getBundledJsonFiles(bundledDir) {
  if (!fs.existsSync(bundledDir)) {
    throw new Error(`Bundled dict directory not found: ${bundledDir}`);
  }
  return fs
    .readdirSync(bundledDir, { withFileTypes: true })
    .filter(
      (entry) =>
        entry.isFile() &&
        entry.name.toLowerCase().endsWith(".json") &&
        entry.name.toLowerCase() !== "dict-orders.json",
    )
    .map((entry) => path.join(bundledDir, entry.name));
}

function buildBundledTermIndex(bundledDir) {
  const files = getBundledJsonFiles(bundledDir);
  const termIndex = new Map();

  for (const filePath of files) {
    const fileName = path.basename(filePath);
    const items = readJsonArray(filePath);
    let dictName = path.basename(filePath, path.extname(filePath));

    const meta = items.find(
      (item) =>
        item !== null &&
        typeof item === "object" &&
        Object.prototype.hasOwnProperty.call(item, "dictName") &&
        typeof item.dictName === "string" &&
        item.dictName.trim().length > 0,
    );
    if (meta) {
      dictName = meta.dictName.trim();
    }

    for (const item of items) {
      if (!isEntryItem(item)) {
        continue;
      }
      const normalized = normalizeTerm(item.term);
      if (!normalized) {
        continue;
      }
      const existing = termIndex.get(normalized);
      if (existing) {
        existing.dictNames.add(dictName);
        existing.files.add(fileName);
        continue;
      }
      termIndex.set(normalized, {
        dictNames: new Set([dictName]),
        files: new Set([fileName]),
      });
    }
  }

  return termIndex;
}

function formatRemovedPreview(removedItems) {
  return removedItems
    .map((item) => {
      const dictNames = [...item.dictNames].sort().join("、");
      return `- ${item.term} (${dictNames})`;
    })
    .join("\n");
}

function cleanCustomDuplicates({ customSource, bundledDir, dryRun, backup }) {
  const customItems = readCustomEntries(customSource);
  const bundledIndex = buildBundledTermIndex(bundledDir);

  const kept = [];
  const removed = [];

  for (const item of customItems) {
    if (!isEntryItem(item)) {
      kept.push(item);
      continue;
    }
    const normalized = normalizeTerm(item.term);
    if (!normalized) {
      kept.push(item);
      continue;
    }
    const bundledHit = bundledIndex.get(normalized);
    if (!bundledHit) {
      kept.push(item);
      continue;
    }
    removed.push({
      term: item.term.trim(),
      dictNames: bundledHit.dictNames,
    });
  }

  const customEntryCount = customItems.filter(isEntryItem).length;
  const output = {
    customPath: customSource.path,
    customKind: customSource.kind,
    bundledDir,
    customEntryCount,
    removedCount: removed.length,
    keptCount: kept.filter(isEntryItem).length,
    removed,
  };

  if (dryRun || removed.length === 0) {
    return output;
  }

  if (backup) {
    const stamp = new Date().toISOString().replaceAll(":", "").replaceAll(".", "");
    const backupPath = `${customSource.path}.${stamp}.bak`;
    fs.copyFileSync(customSource.path, backupPath);
    output.backupPath = backupPath;
  }

  writeCustomEntries(customSource, kept);
  return output;
}

function main() {
  try {
    const options = parseArgs(process.argv.slice(2));
    if (options.help) {
      printHelp();
      return;
    }

    const customSource = resolveCustomSource(options);
    const result = cleanCustomDuplicates({
      customSource,
      bundledDir: options.bundledDir,
      dryRun: options.dryRun,
      backup: options.backup,
    });

    console.log(`custom source: ${result.customPath} (${result.customKind})`);
    console.log(`bundled dict dir: ${result.bundledDir}`);
    console.log(`custom entry count: ${result.customEntryCount}`);
    console.log(`removed duplicates: ${result.removedCount}`);
    console.log(`remaining entries: ${result.keptCount}`);
    if (result.backupPath) {
      console.log(`backup: ${result.backupPath}`);
    }

    if (result.removedCount > 0) {
      console.log("\nremoved terms:");
      console.log(formatRemovedPreview(result.removed));
    }

    if (options.dryRun) {
      console.log("\ndry run mode: no files were changed.");
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}

main();

