import fs from "node:fs";
import path from "node:path";

function printHelp() {
  console.log(`Usage:
  node scripts/clean-custom-bundled-duplicates.mjs [options]

Options:
  --dry-run               Preview removals without writing files
  --settings <path>       Settings file path (default: %APPDATA%/com.local.name-dict/settings.json)
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

function cleanCustomDuplicates({ customPath, bundledDir, dryRun, backup }) {
  const customItems = readJsonArray(customPath);
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
    customPath,
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
    const backupPath = `${customPath}.${stamp}.bak`;
    fs.copyFileSync(customPath, backupPath);
    output.backupPath = backupPath;
  }

  fs.writeFileSync(customPath, `${JSON.stringify(kept, null, 2)}\n`, "utf8");
  return output;
}

function main() {
  try {
    const options = parseArgs(process.argv.slice(2));
    if (options.help) {
      printHelp();
      return;
    }

    const customPath = resolveCustomEntriesPath(options);
    const result = cleanCustomDuplicates({
      customPath,
      bundledDir: options.bundledDir,
      dryRun: options.dryRun,
      backup: options.backup,
    });

    console.log(`custom entries: ${result.customPath}`);
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
