import fs from "node:fs";
import path from "node:path";

const TARGET_CONFIG = {
  "west-female.json": {
    dictId: "west-female",
  },
  "west-male.json": {
    dictId: "west-male",
  },
  "west-surname.json": {
    dictId: "west-surname",
  },
  "east-female.json": {
    dictId: "east-female",
  },
  "east-male.json": {
    dictId: "east-male",
  },
  "west-faction.json": {
    dictId: "west-faction",
  },
  "east-faction.json": {
    dictId: "east-faction",
  },
  "west-place.json": {
    dictId: "west-place",
  },
  "east-place.json": {
    dictId: "east-place",
  },
  "nickname.json": {
    dictId: "nickname",
  },
  "others.json": {
    dictId: "others",
  },
  "items.json": {
    dictId: "items",
  },
};

function printHelp() {
  console.log(`Usage:
  node scripts/export-custom-into-bundled.mjs [options]

Options:
  --dry-run               Preview changes without writing files
  --settings <path>       Settings file path (default: %APPDATA%/com.local.name-dict/settings.json)
  --custom <path>         Custom entries.json path (overrides --settings)
  --dict-dir <path>       Built-in dict directory (default: ./dict)
  --backup                Create backup before writing changed files
  --help                  Show this message
`);
}

function parseArgs(argv) {
  const options = {
    dryRun: false,
    backup: false,
    settingsPath: null,
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
      options.settingsPath = argv[i + 1] ?? null;
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
    throw new Error(`Unknown argument: ${arg}`);
  }

  if (options.settingsPath) {
    options.settingsPath = path.resolve(options.settingsPath);
  }
  if (options.customPath) {
    options.customPath = path.resolve(options.customPath);
  }
  return options;
}

function resolveDefaultSettingsPath() {
  const appData = process.env.APPDATA;
  if (!appData) {
    throw new Error("APPDATA is not available; please provide --settings or --custom.");
  }
  return path.join(appData, "com.local.name-dict", "settings.json");
}

function resolveCustomEntriesPath(options) {
  if (options.customPath) {
    return options.customPath;
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
  return path.join(dictDir, "entries.json");
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

function normalizeNameType(value) {
  return typeof value === "string" ? value.trim().toLowerCase() : "";
}

function normalizeGenderType(value) {
  const normalized = typeof value === "string" ? value.trim().toLowerCase() : "";
  if (normalized === "male" || normalized === "female" || normalized === "both") {
    return normalized;
  }
  return "both";
}

function normalizeGenre(value) {
  return typeof value === "string" ? value.trim().toLowerCase() : "";
}

function detectTargetFile(entry) {
  const nameType = normalizeNameType(entry.nameType);
  const genderType = normalizeGenderType(entry.genderType);
  const genre = normalizeGenre(entry.genre);

  if (nameType === "given" && genderType === "female" && genre === "west") {
    return "west-female.json";
  }
  if (nameType === "given" && genderType === "male" && genre === "west") {
    return "west-male.json";
  }
  if (nameType === "surname" && genre === "west") {
    return "west-surname.json";
  }
  if (nameType === "given" && genderType === "female" && genre === "east") {
    return "east-female.json";
  }
  if (nameType === "given" && genderType === "male" && genre === "east") {
    return "east-male.json";
  }
  if (nameType === "faction" && genre === "west") {
    return "west-faction.json";
  }
  if (nameType === "faction" && genre === "east") {
    return "east-faction.json";
  }
  if (nameType === "place" && genre === "west") {
    return "west-place.json";
  }
  if (nameType === "place" && genre === "east") {
    return "east-place.json";
  }
  if (nameType === "nickname") {
    return "nickname.json";
  }
  if (nameType === "other" || nameType === "others") {
    return "others.json";
  }
  if (nameType === "item" || nameType === "items") {
    return "items.json";
  }
  return null;
}

function normalizeGenreType(value) {
  const normalized = normalizeGenre(value);
  if (normalized === "east" || normalized === "west") {
    return normalized;
  }
  return "west";
}

function toBundledEntry(entry, targetFile) {
  const term = entry.term.trim();
  const group = normalizeGroup(entry.group);

  if (targetFile === "west-surname.json") {
    return {
      term,
      group,
      nameType: "surname",
      genderType: normalizeGenderType(entry.genderType),
      genre: "west",
    };
  }

  if (targetFile === "west-female.json") {
    return { term, group, nameType: "given", genderType: "female", genre: "west" };
  }
  if (targetFile === "west-male.json") {
    return { term, group, nameType: "given", genderType: "male", genre: "west" };
  }
  if (targetFile === "east-female.json") {
    return { term, group, nameType: "given", genderType: "female", genre: "east" };
  }
  if (targetFile === "east-male.json") {
    return { term, group, nameType: "given", genderType: "male", genre: "east" };
  }
  if (targetFile === "west-faction.json") {
    return {
      term,
      group,
      nameType: "faction",
      genderType: normalizeGenderType(entry.genderType),
      genre: "west",
    };
  }
  if (targetFile === "east-faction.json") {
    return {
      term,
      group,
      nameType: "faction",
      genderType: normalizeGenderType(entry.genderType),
      genre: "east",
    };
  }
  if (targetFile === "west-place.json") {
    return {
      term,
      group,
      nameType: "place",
      genderType: normalizeGenderType(entry.genderType),
      genre: "west",
    };
  }
  if (targetFile === "east-place.json") {
    return {
      term,
      group,
      nameType: "place",
      genderType: normalizeGenderType(entry.genderType),
      genre: "east",
    };
  }
  if (targetFile === "nickname.json") {
    return {
      term,
      group,
      nameType: "nickname",
      genderType: normalizeGenderType(entry.genderType),
      genre: normalizeGenreType(entry.genre),
    };
  }
  if (targetFile === "others.json") {
    return {
      term,
      group,
      nameType: "others",
      genderType: normalizeGenderType(entry.genderType),
      genre: normalizeGenreType(entry.genre),
    };
  }
  if (targetFile === "items.json") {
    return {
      term,
      group,
      nameType: "item",
      genderType: normalizeGenderType(entry.genderType),
      genre: normalizeGenreType(entry.genre),
    };
  }

  throw new Error(`Unsupported target file: ${targetFile}`);
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

function ensureTargetFileData(filePath, fileName) {
  if (!fs.existsSync(filePath)) {
    const config = TARGET_CONFIG[fileName];
    return {
      metaItems: [{ dictId: config.dictId }],
      entries: [],
      rawArray: [],
    };
  }

  const rawArray = readJsonArray(filePath);
  const metaItems = [];
  const entries = [];

  for (const item of rawArray) {
    if (isEntryItem(item)) {
      const term = item.term.trim();
      if (!term) {
        continue;
      }
      entries.push({
        ...item,
        term,
        group: normalizeGroup(item.group),
      });
      continue;
    }
    metaItems.push(item);
  }

  if (metaItems.length === 0) {
    const config = TARGET_CONFIG[fileName];
    metaItems.push({ dictId: config.dictId });
  }

  return { metaItems, entries, rawArray };
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

    if (!fs.existsSync(options.dictDir)) {
      throw new Error(`Built-in dict directory not found: ${options.dictDir}`);
    }

    const customPath = resolveCustomEntriesPath(options);
    const customArray = readJsonArray(customPath);
    const movedBuckets = new Map(Object.keys(TARGET_CONFIG).map((fileName) => [fileName, []]));
    const keptCustomItems = [];
    let movedCount = 0;
    let unmatchedEntryCount = 0;

    for (const item of customArray) {
      if (!isEntryItem(item)) {
        keptCustomItems.push(item);
        continue;
      }

      const targetFile = detectTargetFile(item);
      if (!targetFile) {
        unmatchedEntryCount += 1;
        keptCustomItems.push(item);
        continue;
      }

      const term = item.term.trim();
      if (!term) {
        unmatchedEntryCount += 1;
        keptCustomItems.push(item);
        continue;
      }

      movedBuckets.get(targetFile).push(toBundledEntry(item, targetFile));
      movedCount += 1;
    }

    const targetResults = [];
    const changedFiles = [];
    const backupFiles = new Set();
    const backupStamp = new Date().toISOString().replaceAll(":", "").replaceAll(".", "");
    let duplicateSkippedCount = 0;

    for (const [fileName, movedEntries] of movedBuckets.entries()) {
      const filePath = path.join(options.dictDir, fileName);
      const { metaItems, entries } = ensureTargetFileData(filePath, fileName);
      const merged = [...entries];
      const seen = new Set(entries.map((entry) => normalizeTerm(entry.term)));
      let added = 0;

      for (const moved of movedEntries) {
        const key = normalizeTerm(moved.term);
        if (!key) {
          continue;
        }
        if (seen.has(key)) {
          duplicateSkippedCount += 1;
          continue;
        }
        seen.add(key);
        merged.push(moved);
        added += 1;
      }

      merged.sort(compareByGroupThenTerm);
      const outputArray = [...metaItems, ...merged];
      const nextContent = formatJsonArray(outputArray);
      const prevContent = fs.existsSync(filePath) ? fs.readFileSync(filePath, "utf8") : "";
      const changed = nextContent !== prevContent;

      targetResults.push({
        fileName,
        filePath,
        movedIn: movedEntries.length,
        added,
        changed,
        nextContent,
      });

      if (changed) {
        changedFiles.push(filePath);
      }
    }

    const nextCustomContent = formatJsonArray(keptCustomItems);
    const prevCustomContent = fs.readFileSync(customPath, "utf8");
    const customChanged = nextCustomContent !== prevCustomContent;
    if (customChanged) {
      changedFiles.push(customPath);
    }

    if (!options.dryRun) {
      for (const result of targetResults) {
        if (!result.changed) {
          continue;
        }
        backupFileIfNeeded(result.filePath, options.backup, backupStamp, backupFiles);
        fs.writeFileSync(result.filePath, result.nextContent, "utf8");
      }

      if (customChanged) {
        backupFileIfNeeded(customPath, options.backup, backupStamp, backupFiles);
        fs.writeFileSync(customPath, nextCustomContent, "utf8");
      }
    }

    console.log(`custom entries: ${customPath}`);
    console.log(`built-in dict dir: ${options.dictDir}`);
    console.log(`matched and moved by rules: ${movedCount}`);
    console.log(`unmatched entries kept in custom: ${unmatchedEntryCount}`);
    console.log(`duplicates skipped in target files: ${duplicateSkippedCount}`);
    console.log(`changed files: ${changedFiles.length}`);
    if (options.backup && backupFiles.size > 0) {
      console.log(`backup files: ${backupFiles.size}`);
    }

    for (const result of targetResults) {
      console.log(
        `${result.fileName}: matched ${result.movedIn}, added ${result.added}, ${result.changed ? "updated" : "unchanged"}`,
      );
    }
    console.log(`custom entries.json: ${customChanged ? "updated" : "unchanged"}`);

    if (options.dryRun) {
      console.log("dry run mode: no files were changed.");
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}

main();
