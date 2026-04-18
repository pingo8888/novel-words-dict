import fs from "node:fs";
import path from "node:path";

function printHelp() {
  console.log(`Usage:
  node scripts/bump-version.mjs <version> [options]

Options:
  --dry-run        Preview changes without writing files
  --no-lock        Do not update src-tauri/Cargo.lock
  --help           Show this message
`);
}

function parseArgs(argv) {
  const options = {
    version: "",
    dryRun: false,
    updateLock: true,
    help: false,
  };

  for (const arg of argv) {
    if (arg === "--dry-run") {
      options.dryRun = true;
      continue;
    }
    if (arg === "--no-lock") {
      options.updateLock = false;
      continue;
    }
    if (arg === "--help" || arg === "-h") {
      options.help = true;
      continue;
    }
    if (arg.startsWith("--")) {
      throw new Error(`Unknown option: ${arg}`);
    }
    if (!options.version) {
      options.version = arg.trim();
      continue;
    }
    throw new Error(`Unexpected argument: ${arg}`);
  }

  return options;
}

function assertVersion(version) {
  const semver =
    /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/;
  if (!semver.test(version)) {
    throw new Error(`Invalid version: ${version}`);
  }
}

function updateJsonVersion(filePath, version) {
  const text = fs.readFileSync(filePath, "utf8");
  const json = JSON.parse(text);
  if (typeof json !== "object" || json === null) {
    throw new Error(`Expected JSON object: ${filePath}`);
  }
  const prev = typeof json.version === "string" ? json.version : "";
  json.version = version;
  const nextText = `${JSON.stringify(json, null, 2)}\n`;
  return { prev, nextText };
}

function updateCargoTomlVersion(filePath, version) {
  const text = fs.readFileSync(filePath, "utf8");
  const lines = text.split(/\r?\n/);
  let inPackage = false;
  let replaced = false;
  let prev = "";

  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    const sectionMatch = /^\s*\[(.+)\]\s*$/.exec(line);
    if (sectionMatch) {
      inPackage = sectionMatch[1] === "package";
      continue;
    }
    if (!inPackage) {
      continue;
    }
    const versionMatch = /^(\s*version\s*=\s*)"(.*)"(\s*)$/.exec(line);
    if (!versionMatch) {
      continue;
    }
    prev = versionMatch[2];
    lines[i] = `${versionMatch[1]}"${version}"${versionMatch[3]}`;
    replaced = true;
    break;
  }

  if (!replaced) {
    throw new Error(`Could not find [package].version in ${filePath}`);
  }

  return { prev, nextText: `${lines.join("\n")}\n` };
}

function updateCargoLockRootVersion(filePath, packageName, version) {
  const text = fs.readFileSync(filePath, "utf8");
  const lines = text.split(/\r?\n/);
  let inTargetBlock = false;
  let replaced = false;
  let prev = "";

  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    if (/^\s*\[\[package\]\]\s*$/.test(line)) {
      inTargetBlock = false;
      continue;
    }

    if (!inTargetBlock) {
      const nameMatch = /^\s*name\s*=\s*"(.*)"\s*$/.exec(line);
      if (nameMatch && nameMatch[1] === packageName) {
        inTargetBlock = true;
      }
      continue;
    }

    const versionMatch = /^(\s*version\s*=\s*)"(.*)"(\s*)$/.exec(line);
    if (!versionMatch) {
      continue;
    }
    prev = versionMatch[2];
    lines[i] = `${versionMatch[1]}"${version}"${versionMatch[3]}`;
    replaced = true;
    break;
  }

  if (!replaced) {
    throw new Error(`Could not find package ${packageName} in ${filePath}`);
  }

  return { prev, nextText: `${lines.join("\n")}\n` };
}

function main() {
  try {
    const options = parseArgs(process.argv.slice(2));
    if (options.help) {
      printHelp();
      return;
    }
    if (!options.version) {
      throw new Error("Missing version. Example: node scripts/bump-version.mjs 0.2.0");
    }
    assertVersion(options.version);

    const rootDir = process.cwd();
    const targets = [
      {
        filePath: path.join(rootDir, "package.json"),
        updater: updateJsonVersion,
        label: "package.json",
      },
      {
        filePath: path.join(rootDir, "src-tauri", "tauri.conf.json"),
        updater: updateJsonVersion,
        label: "src-tauri/tauri.conf.json",
      },
      {
        filePath: path.join(rootDir, "src-tauri", "Cargo.toml"),
        updater: updateCargoTomlVersion,
        label: "src-tauri/Cargo.toml",
      },
    ];

    if (options.updateLock) {
      const lockPath = path.join(rootDir, "src-tauri", "Cargo.lock");
      if (fs.existsSync(lockPath)) {
        targets.push({
          filePath: lockPath,
          updater: (filePath, version) =>
            updateCargoLockRootVersion(filePath, "novel-words-dict", version),
          label: "src-tauri/Cargo.lock",
        });
      }
    }

    const results = [];
    for (const target of targets) {
      const { prev, nextText } = target.updater(target.filePath, options.version);
      const oldText = fs.readFileSync(target.filePath, "utf8");
      const changed = oldText !== nextText;
      results.push({ ...target, prev, changed, nextText });
    }

    for (const result of results) {
      const before = result.prev || "(none)";
      const status = result.changed ? "updated" : "unchanged";
      console.log(`${result.label}: ${status} (${before} -> ${options.version})`);
      if (result.changed && !options.dryRun) {
        fs.writeFileSync(result.filePath, result.nextText, "utf8");
      }
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
