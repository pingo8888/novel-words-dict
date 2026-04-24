import fs from "node:fs";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";

const DEFAULT_ORDER = 2147483647;

function printHelp() {
  console.log(`Usage:
  node scripts/build-builtin-db.mjs [options]

Options:
  --dict-dir <path>   Built-in dict source directory (default: ./dict)
  --out <path>        Output sqlite file path (default: ./build-in.db)
  --empty             Build an empty database without reading dict JSON files
  --skip-dict-json    Alias for --empty
  --verify            Validate source only, do not write output file
  --help              Show this message
`);
}

function parseArgs(argv) {
  const options = {
    dictDir: path.resolve(process.cwd(), "dict"),
    outPath: path.resolve(process.cwd(), "build-in.db"),
    empty: false,
    verify: false,
    help: false,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--help" || arg === "-h") {
      options.help = true;
      continue;
    }
    if (arg === "--verify") {
      options.verify = true;
      continue;
    }
    if (arg === "--empty" || arg === "--skip-dict-json") {
      options.empty = true;
      continue;
    }
    if (arg === "--dict-dir") {
      options.dictDir = path.resolve(argv[i + 1] ?? "");
      i += 1;
      continue;
    }
    if (arg === "--out") {
      options.outPath = path.resolve(argv[i + 1] ?? "");
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

function normalizeNameType(value) {
  const normalized = typeof value === "string" ? value.trim().toLowerCase() : "";
  const allowed = new Set([
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
  if (normalized === "items") {
    return "item";
  }
  if (normalized === "other" || normalized === "incantation") {
    return "others";
  }
  if (allowed.has(normalized)) {
    return normalized;
  }
  return "both";
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

function isEntry(item) {
  return (
    item !== null &&
    typeof item === "object" &&
    Object.prototype.hasOwnProperty.call(item, "term") &&
    typeof item.term === "string"
  );
}

function parseDictMeta(item) {
  if (!item || typeof item !== "object") {
    return null;
  }
  const rawDictId =
    (typeof item.dictId === "string" ? item.dictId : null) ??
    (typeof item.dict_id === "string" ? item.dict_id : null) ??
    "";
  const dictId = sanitizeDictId(rawDictId);
  if (!dictId) {
    return null;
  }
  const rawDictName =
    (typeof item.dictName === "string" ? item.dictName : null) ??
    (typeof item.dict_name === "string" ? item.dict_name : null) ??
    "";
  const dictName = rawDictName.trim();
  const rawOrder =
    typeof item.order === "number" && Number.isFinite(item.order)
      ? Math.trunc(item.order)
      : null;
  const order =
    rawOrder !== null && rawOrder >= -2147483648 && rawOrder <= 2147483647 ? rawOrder : null;
  return { dictId, dictName, order };
}

function loadJsonArray(filePath) {
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

function loadDictOrderConfig(dictDir) {
  const configPath = path.join(dictDir, "dict-orders.json");
  if (!fs.existsSync(configPath)) {
    return new Map();
  }
  const raw = fs.readFileSync(configPath, "utf8").trim();
  if (!raw) {
    return new Map();
  }
  const parsed = JSON.parse(raw);
  const out = new Map();

  if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
    for (const [key, value] of Object.entries(parsed)) {
      const id = sanitizeDictId(key);
      if (!id) {
        continue;
      }
      if (typeof value === "number" && Number.isFinite(value)) {
        out.set(id, { order: Math.trunc(value), dictName: null });
        continue;
      }
      if (!value || typeof value !== "object" || Array.isArray(value)) {
        continue;
      }
      const order =
        typeof value.order === "number" && Number.isFinite(value.order)
          ? Math.trunc(value.order)
          : null;
      const dictName =
        typeof value.dictName === "string"
          ? value.dictName.trim()
          : typeof value.dict_name === "string"
            ? value.dict_name.trim()
            : "";
      if (order === null && !dictName) {
        continue;
      }
      out.set(id, { order, dictName: dictName || null });
    }
    return out;
  }

  if (Array.isArray(parsed)) {
    for (const item of parsed) {
      if (!item || typeof item !== "object" || Array.isArray(item)) {
        continue;
      }
      const rawId =
        (typeof item.dictId === "string" ? item.dictId : null) ??
        (typeof item.dict_id === "string" ? item.dict_id : null) ??
        "";
      const id = sanitizeDictId(rawId);
      if (!id) {
        continue;
      }
      const order =
        typeof item.order === "number" && Number.isFinite(item.order)
          ? Math.trunc(item.order)
          : null;
      const dictName =
        typeof item.dictName === "string"
          ? item.dictName.trim()
          : typeof item.dict_name === "string"
            ? item.dict_name.trim()
            : "";
      if (order === null && !dictName) {
        continue;
      }
      out.set(id, { order, dictName: dictName || null });
    }
  }

  return out;
}

function buildBuckets(dictDir, options = {}) {
  if (options.empty) {
    return [];
  }

  if (!fs.existsSync(dictDir)) {
    throw new Error(`Dict directory not found: ${dictDir}`);
  }

  const files = fs
    .readdirSync(dictDir, { withFileTypes: true })
    .filter(
      (entry) =>
        entry.isFile() &&
        entry.name.toLowerCase().endsWith(".json") &&
        entry.name.toLowerCase() !== "dict-orders.json",
    )
    .map((entry) => entry.name)
    .sort((a, b) => a.localeCompare(b));

  const configMap = loadDictOrderConfig(dictDir);
  const grouped = new Map();

  files.forEach((fileName, fileIndex) => {
    const filePath = path.join(dictDir, fileName);
    const items = loadJsonArray(filePath);

    let meta = null;
    const entries = [];
    for (let i = 0; i < items.length; i += 1) {
      const item = items[i];
      if (i === 0) {
        const parsedMeta = parseDictMeta(item);
        if (parsedMeta) {
          meta = parsedMeta;
          continue;
        }
      }
      if (!isEntry(item)) {
        continue;
      }
      const term = item.term.trim();
      if (!term) {
        continue;
      }
      entries.push({
        term,
        group: typeof item.group === "string" ? item.group.trim() : "",
        nameType: normalizeNameType(item.nameType),
        genderType: normalizeGenderType(item.genderType),
        genre: normalizeGenre(item.genre),
      });
    }

    const fallbackId = path.basename(fileName, path.extname(fileName)).trim() || "bundled";
    let id = sanitizeDictId(meta?.dictId ?? "") || sanitizeDictId(fallbackId);
    if (!id || id === "custom") {
      id = `bundled-${sanitizeDictId(fallbackId) || "dict"}`;
    }

    const declaredOrder = Number.isInteger(meta?.order) ? meta.order : DEFAULT_ORDER;
    const conf = configMap.get(id);
    const resolvedOrder = Number.isInteger(conf?.order) ? conf.order : declaredOrder;

    const fallbackName = (meta?.dictName ?? "").trim() || fallbackId;
    const resolvedName = (conf?.dictName ?? "").trim() || fallbackName;

    if (grouped.has(id)) {
      const existing = grouped.get(id);
      const sameMeta = existing.order === resolvedOrder && existing.name === resolvedName;
      if (sameMeta) {
        existing.entries.push(...entries);
        return;
      }
    }

    let resolvedId = id;
    while (grouped.has(resolvedId)) {
      resolvedId += "1";
    }

    grouped.set(resolvedId, {
      id: resolvedId,
      name: resolvedName,
      order: resolvedOrder,
      fileIndex,
      entries,
    });
  });

  return [...grouped.values()].sort((a, b) => {
    if (a.order !== b.order) {
      return a.order - b.order;
    }
    if (a.fileIndex !== b.fileIndex) {
      return a.fileIndex - b.fileIndex;
    }
    return a.id.localeCompare(b.id);
  });
}

function writeDb(outPath, buckets) {
  const parent = path.dirname(outPath);
  fs.mkdirSync(parent, { recursive: true });
  const tempPath = `${outPath}.tmp`;
  if (fs.existsSync(tempPath)) {
    fs.rmSync(tempPath, { force: true });
  }

  const db = new DatabaseSync(tempPath);
  try {
    db.exec(`
      PRAGMA foreign_keys = ON;
      CREATE TABLE dictionaries (
        dict_id TEXT PRIMARY KEY,
        dict_name TEXT NOT NULL,
        sort_order INTEGER NOT NULL,
        file_index INTEGER NOT NULL
      );
      CREATE TABLE entries (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        dict_id TEXT NOT NULL,
        term TEXT NOT NULL,
        group_name TEXT NOT NULL DEFAULT '',
        name_type TEXT NOT NULL,
        gender_type TEXT NOT NULL,
        genre TEXT NOT NULL,
        FOREIGN KEY (dict_id) REFERENCES dictionaries(dict_id) ON DELETE CASCADE
      );
      CREATE INDEX idx_entries_dict ON entries(dict_id);
      CREATE INDEX idx_entries_term ON entries(term);
    `);

    const insertDict = db.prepare(
      "INSERT INTO dictionaries (dict_id, dict_name, sort_order, file_index) VALUES (?, ?, ?, ?)",
    );
    const insertEntry = db.prepare(
      "INSERT INTO entries (dict_id, term, group_name, name_type, gender_type, genre) VALUES (?, ?, ?, ?, ?, ?)",
    );

    db.exec("BEGIN");
    for (const bucket of buckets) {
      insertDict.run(bucket.id, bucket.name, bucket.order, bucket.fileIndex);
      for (const entry of bucket.entries) {
        insertEntry.run(
          bucket.id,
          entry.term,
          entry.group,
          entry.nameType,
          entry.genderType,
          entry.genre,
        );
      }
    }
    db.exec("COMMIT");
  } catch (error) {
    try {
      db.exec("ROLLBACK");
    } catch {
      // ignore rollback error
    }
    throw error;
  } finally {
    db.close();
  }

  if (fs.existsSync(outPath)) {
    fs.rmSync(outPath, { force: true });
  }
  fs.renameSync(tempPath, outPath);
}

function main() {
  try {
    const options = parseArgs(process.argv.slice(2));
    if (options.help) {
      printHelp();
      return;
    }

    const buckets = buildBuckets(options.dictDir, { empty: options.empty });
    const dictCount = buckets.length;
    const entryCount = buckets.reduce((acc, bucket) => acc + bucket.entries.length, 0);

    if (options.verify) {
      console.log(`verify ok: dictionaries=${dictCount}, entries=${entryCount}`);
      return;
    }

    writeDb(options.outPath, buckets);
    console.log(`built-in db: ${options.outPath}`);
    console.log(`dictionaries: ${dictCount}`);
    console.log(`entries: ${entryCount}`);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}

main();
