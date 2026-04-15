import fs from "node:fs";
import path from "node:path";

function printHelp() {
  console.log(`Usage:
  node scripts/normalize-bundled-dicts.mjs [options]

Options:
  --dry-run             Preview changes without writing files
  --dict-dir <path>     Built-in dict directory (default: ./dict)
  --help                Show this message
`);
}

function parseArgs(argv) {
  const options = {
    dryRun: false,
    dictDir: path.resolve(process.cwd(), "dict"),
    help: false,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--dry-run") {
      options.dryRun = true;
      continue;
    }
    if (arg === "--dict-dir") {
      const next = argv[i + 1];
      if (!next) {
        throw new Error("Missing value for --dict-dir");
      }
      options.dictDir = path.resolve(next);
      i += 1;
      continue;
    }
    if (arg === "--help" || arg === "-h") {
      options.help = true;
      continue;
    }
    throw new Error(`Unknown argument: ${arg}`);
  }

  return options;
}

function isEntryItem(item) {
  return (
    item !== null &&
    typeof item === "object" &&
    Object.prototype.hasOwnProperty.call(item, "term") &&
    typeof item.term === "string"
  );
}

function normalizeTermKey(term) {
  return term.trim().toLowerCase();
}

const pinyinCollator = new Intl.Collator("zh-Hans-u-co-pinyin", {
  sensitivity: "base",
  numeric: true,
});

function compareTermsByPinyin(left, right) {
  const leftTerm = left.term.trim();
  const rightTerm = right.term.trim();
  const byPinyin = pinyinCollator.compare(leftTerm, rightTerm);
  if (byPinyin !== 0) {
    return byPinyin;
  }
  return leftTerm.localeCompare(rightTerm, "zh-Hans");
}

function formatJsonArray(items) {
  if (items.length === 0) {
    return "[]\n";
  }
  return `[\n${items.map((item) => JSON.stringify(item)).join(",\n")}\n]\n`;
}

function processFile(filePath) {
  const raw = fs.readFileSync(filePath, "utf8").trim();
  const parsed = raw ? JSON.parse(raw) : [];
  if (!Array.isArray(parsed)) {
    throw new Error(`Expected JSON array: ${filePath}`);
  }

  const metaItems = [];
  const termEntries = [];

  for (const item of parsed) {
    if (isEntryItem(item)) {
      const trimmedTerm = item.term.trim();
      termEntries.push({
        ...item,
        term: trimmedTerm,
      });
      continue;
    }
    metaItems.push(item);
  }

  const unique = [];
  const seen = new Set();
  let duplicateCount = 0;

  for (const entry of termEntries) {
    const key = normalizeTermKey(entry.term);
    if (!key) {
      unique.push(entry);
      continue;
    }
    if (seen.has(key)) {
      duplicateCount += 1;
      continue;
    }
    seen.add(key);
    unique.push(entry);
  }

  unique.sort(compareTermsByPinyin);

  const nextArray = [...metaItems, ...unique];
  const nextContent = formatJsonArray(nextArray);
  const changed = nextContent !== `${raw}\n` && nextContent !== raw;

  return {
    filePath,
    changed,
    duplicateCount,
    beforeEntryCount: termEntries.length,
    afterEntryCount: unique.length,
    nextContent,
  };
}

function main() {
  try {
    const options = parseArgs(process.argv.slice(2));
    if (options.help) {
      printHelp();
      return;
    }

    if (!fs.existsSync(options.dictDir)) {
      throw new Error(`Dict directory not found: ${options.dictDir}`);
    }

    const files = fs
      .readdirSync(options.dictDir, { withFileTypes: true })
      .filter((entry) => entry.isFile() && entry.name.toLowerCase().endsWith(".json"))
      .map((entry) => path.join(options.dictDir, entry.name))
      .sort((a, b) => a.localeCompare(b, "en"));

    if (files.length === 0) {
      console.log(`No json files found in: ${options.dictDir}`);
      return;
    }

    const results = files.map((filePath) => processFile(filePath));

    let changedFiles = 0;
    let totalDuplicatesRemoved = 0;

    for (const result of results) {
      const fileName = path.basename(result.filePath);
      const status = result.changed ? "updated" : "unchanged";
      console.log(
        `${fileName}: ${status}, entries ${result.beforeEntryCount} -> ${result.afterEntryCount}, removed ${result.duplicateCount}`,
      );

      if (result.duplicateCount > 0) {
        totalDuplicatesRemoved += result.duplicateCount;
      }

      if (result.changed) {
        changedFiles += 1;
        if (!options.dryRun) {
          fs.writeFileSync(result.filePath, result.nextContent, "utf8");
        }
      }
    }

    console.log(`\nfiles processed: ${results.length}`);
    console.log(`files changed: ${changedFiles}`);
    console.log(`duplicates removed: ${totalDuplicatesRemoved}`);
    if (options.dryRun) {
      console.log("dry run mode: no files were changed.");
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}

main();
