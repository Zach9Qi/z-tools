/**
 * 版本号读写与一致性校验。
 *
 * 版本号散落在 package.json / src-tauri/Cargo.toml / src-tauri/tauri.conf.json5 三处,
 * 另外 src-tauri/Cargo.lock 也记录本 crate 的版本。本模块统一负责:
 * - readVersions:按固定正则读取三处(及 Cargo.lock)的版本号
 * - writeVersions:纯文本替换写入三处,保留 tauri.conf.json5 的 JSON5 注释与 Cargo.toml 的其余内容
 * - refreshCargoLock:Cargo.toml 改完后刷新 Cargo.lock 中本 crate 的版本,不下载不编译
 * - assertConsistent:校验三处一致,可选与 tag 比对
 * - CLI:`bun run scripts/version.ts check [vX.Y.Z]`,供 release.yml 的 verify job 与本地自检复用
 *
 * 读与写共用同一组正则,保证「读到什么就改什么」;不用 JSON.parse / TOML 解析器重序列化,
 * 否则 tauri.conf.json5 的注释会丢。只用 Node 内置模块,node / bun 皆可运行。
 */
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/** 仓库根目录(scripts/ 的上一级),模块内所有相对路径都以它为基准 */
export const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

/** 记录版本号的三个源文件(相对仓库根) */
export const VERSION_FILES = {
  packageJson: "package.json",
  cargoToml: "src-tauri/Cargo.toml",
  tauriConf: "src-tauri/tauri.conf.json5",
} as const;

/** Cargo.lock 路径(相对仓库根);由 cargo 刷新,脚本不手写 */
export const CARGO_LOCK = "src-tauri/Cargo.lock";

/** cargo 命令的执行目录(Cargo.toml 所在目录) */
const CARGO_DIR = "src-tauri";

/** 版本号相关的校验 / 读写错误;message 是面向用户的中文说明,CLI 直接打印 */
export class VersionError extends Error {}

/** 三处版本号 + Cargo.lock 的快照 */
export interface VersionSnapshot {
  /** package.json 根级 version */
  packageJson: string;
  /** src-tauri/Cargo.toml [package] 段的 version */
  cargoToml: string;
  /** src-tauri/tauri.conf.json5 根级 version */
  tauriConf: string;
  /** src-tauri/Cargo.lock 中本 crate 的 version;找不到条目时为 undefined */
  cargoLock: string | undefined;
}

/** 版本递增类型 */
export type BumpKind = "patch" | "minor" | "major";

/** 全部递增类型,供参数解析与用法提示使用 */
export const BUMP_KINDS: readonly BumpKind[] = ["patch", "minor", "major"];

/** semver 格式:x.y.z,可带 `-预发布后缀`(如 1.2.0-beta.1) */
const SEMVER_RE = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/;

/**
 * JSON / JSON5 文件里的 `"version": "..."` 字段。/m 下首个匹配就是根级 version:
 * package.json 与 tauri.conf.json5 的根级 version 都排在任何嵌套对象之前,依赖表里也没有 version 键。
 */
const JSON_VERSION_RE = /^(\s*"version":\s*")([^"]+)(")/m;

/** Cargo.toml 里的 `version = "..."`,只在 [package] 段的切片内使用,不会误伤依赖声明 */
const TOML_VERSION_RE = /^(version\s*=\s*")([^"]+)(")/m;

/** Cargo.toml [package] 段的 `name = "..."`,用于在 Cargo.lock 里定位本 crate */
const TOML_NAME_RE = /^name\s*=\s*"([^"]+)"/m;

/** 判断字符串是否为合法 semver 版本号 */
export function isValidVersion(version: string): boolean {
  return SEMVER_RE.test(version);
}

/** 判断字符串是否为递增类型(patch / minor / major) */
export function isBumpKind(value: string): value is BumpKind {
  return (BUMP_KINDS as readonly string[]).includes(value);
}

/**
 * 基于 current 递增版本号。预发布后缀会被丢弃再递增(0.2.0-beta.1 patch → 0.2.1),
 * 刻意不实现 npm semver 那套「预发布转正」语义,发布预发布版请直接传完整版本号。
 */
export function bumpVersion(current: string, kind: BumpKind): string {
  if (!isValidVersion(current)) {
    throw new VersionError(`当前版本号 ${current} 不是合法的 semver,无法递增`);
  }
  const [major, minor, patch] = current.split("-")[0].split(".").map(Number);
  switch (kind) {
    case "major":
      return `${major + 1}.0.0`;
    case "minor":
      return `${major}.${minor + 1}.0`;
    case "patch":
      return `${major}.${minor}.${patch + 1}`;
  }
}

/** 去掉 tag 的 `v` 前缀得到版本号;格式不合法时抛错 */
export function versionFromTag(tag: string): string {
  const version = tag.startsWith("v") ? tag.slice(1) : tag;
  if (!isValidVersion(version)) {
    throw new VersionError(`tag ${tag} 不是合法格式,期望 vX.Y.Z(可带 -预发布后缀)`);
  }
  return version;
}

/** 读取仓库内文件(UTF-8);文件缺失等 IO 错误转成带中文说明的 VersionError,CLI 不至于吐出裸堆栈 */
function readRepoFile(relPath: string): string {
  try {
    return readFileSync(resolve(REPO_ROOT, relPath), "utf8");
  } catch (error) {
    throw new VersionError(`读取 ${relPath} 失败:${describeError(error)}`);
  }
}

/** 写回仓库内文件;内容原样写入,不改动换行风格 */
function writeRepoFile(relPath: string, content: string): void {
  writeFileSync(resolve(REPO_ROOT, relPath), content, "utf8");
}

/**
 * 定位 Cargo.toml 的 [package] 段:返回段体的起止下标(不含段头行)。
 * 段尾取下一个以 `[` 开头的段头,没有则到文件末尾。
 */
function locatePackageSection(toml: string): { start: number; end: number } {
  const header = /^\[package\][^\n]*\n/m.exec(toml);
  if (!header) {
    throw new VersionError(`${VERSION_FILES.cargoToml} 缺少 [package] 段`);
  }
  const start = header.index + header[0].length;
  const next = /^\[/m.exec(toml.slice(start));
  return { start, end: next ? start + next.index : toml.length };
}

/** 从 content 中抓取 re 首个匹配的第 2 个捕获组;找不到抛错 */
function capture(content: string, re: RegExp, label: string): string {
  const match = re.exec(content);
  if (!match) {
    throw new VersionError(`${label} 中找不到版本号字段`);
  }
  return match[2];
}

/** 把 content 中 re 首个匹配的第 2 个捕获组替换为 replacement,其余文本原样保留 */
function replaceCaptured(content: string, re: RegExp, replacement: string, label: string): string {
  const match = re.exec(content);
  if (!match) {
    throw new VersionError(`${label} 中找不到版本号字段`);
  }
  const before = content.slice(0, match.index);
  const after = content.slice(match.index + match[0].length);
  return `${before}${match[1]}${replacement}${match[3]}${after}`;
}

/** 读取 Cargo.toml [package] 段的 crate 名 */
function readCrateName(toml: string): string {
  const { start, end } = locatePackageSection(toml);
  const match = TOML_NAME_RE.exec(toml.slice(start, end));
  if (!match) {
    throw new VersionError(`${VERSION_FILES.cargoToml} 的 [package] 段缺少 name`);
  }
  return match[1];
}

/** 读取 Cargo.lock 中指定 crate 的版本;lock 里 name 与 version 相邻成行,直接按行匹配 */
function readCargoLockVersion(crateName: string): string | undefined {
  const lock = readRepoFile(CARGO_LOCK);
  const escaped = crateName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = new RegExp(`^name = "${escaped}"\\r?\\nversion = "([^"]+)"`, "m").exec(lock);
  return match?.[1];
}

/** 读取三处版本号与 Cargo.lock 中本 crate 的版本 */
export function readVersions(): VersionSnapshot {
  const packageJson = capture(
    readRepoFile(VERSION_FILES.packageJson),
    JSON_VERSION_RE,
    VERSION_FILES.packageJson,
  );
  const toml = readRepoFile(VERSION_FILES.cargoToml);
  const { start, end } = locatePackageSection(toml);
  const cargoToml = capture(toml.slice(start, end), TOML_VERSION_RE, VERSION_FILES.cargoToml);
  const tauriConf = capture(
    readRepoFile(VERSION_FILES.tauriConf),
    JSON_VERSION_RE,
    VERSION_FILES.tauriConf,
  );
  return {
    packageJson,
    cargoToml,
    tauriConf,
    cargoLock: readCargoLockVersion(readCrateName(toml)),
  };
}

/**
 * 把三处版本号统一写为 version。
 * 纯文本替换:tauri.conf.json5 的 JSON5 注释、Cargo.toml 其余段落原样保留;不触碰 Cargo.lock。
 */
export function writeVersions(version: string): void {
  if (!isValidVersion(version)) {
    throw new VersionError(`版本号 ${version} 不是合法的 semver(期望 x.y.z,可带 -预发布后缀)`);
  }

  const pkg = readRepoFile(VERSION_FILES.packageJson);
  writeRepoFile(
    VERSION_FILES.packageJson,
    replaceCaptured(pkg, JSON_VERSION_RE, version, VERSION_FILES.packageJson),
  );

  const toml = readRepoFile(VERSION_FILES.cargoToml);
  const { start, end } = locatePackageSection(toml);
  const section = replaceCaptured(
    toml.slice(start, end),
    TOML_VERSION_RE,
    version,
    VERSION_FILES.cargoToml,
  );
  writeRepoFile(VERSION_FILES.cargoToml, `${toml.slice(0, start)}${section}${toml.slice(end)}`);

  const conf = readRepoFile(VERSION_FILES.tauriConf);
  writeRepoFile(
    VERSION_FILES.tauriConf,
    replaceCaptured(conf, JSON_VERSION_RE, version, VERSION_FILES.tauriConf),
  );
}

/**
 * 刷新 Cargo.lock 中本 crate 的版本条目。
 * 用 `cargo update --workspace --offline`:只重新解析工作区成员,不下载、不编译,也不会顺带升级依赖
 * (`cargo metadata --no-deps` 不会写 lock,完整 `cargo metadata` 离线时又要拉平台专属依赖,都不合用)。
 * cargo 不可用或执行失败时打印警告并返回 false,由调用方决定是否继续,不中断发布流程。
 */
export function refreshCargoLock(): boolean {
  try {
    execFileSync("cargo", ["update", "--workspace", "--offline"], {
      cwd: resolve(REPO_ROOT, CARGO_DIR),
      stdio: ["ignore", "ignore", "pipe"],
    });
    return true;
  } catch (error) {
    console.warn(
      `警告:刷新 ${CARGO_LOCK} 失败,请手动在 ${CARGO_DIR}/ 下执行 cargo update --workspace 后再提交。原因:${describeError(error)}`,
    );
    return false;
  }
}

/**
 * 校验三处版本号一致;传入 tag(vX.Y.Z 或 X.Y.Z)时同时校验 tag 与版本号一致。
 * 通过返回快照,不通过抛出带中文说明的 VersionError。
 * Cargo.lock 不参与判定:cargo build 会自动修正它,不值得让发布卡住;由 CLI 单独给警告。
 */
export function assertConsistent(tag?: string): VersionSnapshot {
  const snapshot = readVersions();
  const { packageJson, cargoToml, tauriConf } = snapshot;
  if (packageJson !== cargoToml || packageJson !== tauriConf) {
    throw new VersionError(
      `三处版本号不一致:${VERSION_FILES.packageJson}=${packageJson},${VERSION_FILES.cargoToml}=${cargoToml},${VERSION_FILES.tauriConf}=${tauriConf}。请运行 bun run release <版本> 统一写入`,
    );
  }
  if (tag !== undefined) {
    const expected = versionFromTag(tag);
    if (expected !== packageJson) {
      throw new VersionError(
        `tag ${tag} 与代码版本 ${packageJson} 不一致。请先运行 bun run release ${expected} 同步版本号,再推送 tag`,
      );
    }
  }
  return snapshot;
}

/** 把子进程 / 未知错误整理成一行可读文本:优先取 stderr 首行,其次 Error.message */
export function describeError(error: unknown): string {
  if (error && typeof error === "object" && "stderr" in error) {
    const stderr = String((error as { stderr: unknown }).stderr ?? "").trim();
    if (stderr) return stderr.split("\n")[0];
  }
  return error instanceof Error ? error.message : String(error);
}

/** CLI 入口:`check [tag]`;返回进程退出码 */
function runCli(argv: string[]): number {
  const [command, tag, ...rest] = argv;
  if (command !== "check" || rest.length > 0) {
    console.error("用法:bun run version:check [vX.Y.Z]");
    return 1;
  }
  try {
    const snapshot = assertConsistent(tag);
    console.log(
      `版本一致:${snapshot.packageJson}(${VERSION_FILES.packageJson} / ${VERSION_FILES.cargoToml} / ${VERSION_FILES.tauriConf})`,
    );
    if (tag !== undefined) {
      console.log(`tag ${tag} 与代码版本一致`);
    }
    if (snapshot.cargoLock !== snapshot.packageJson) {
      console.warn(
        `警告:${CARGO_LOCK} 中本 crate 版本为 ${snapshot.cargoLock ?? "(未找到)"},与代码版本不一致;cargo build 会自动修正,建议在 ${CARGO_DIR}/ 下执行 cargo update --workspace 后一并提交`,
      );
    }
    return 0;
  } catch (error) {
    if (error instanceof VersionError) {
      console.error(`版本校验失败:${error.message}`);
      return 1;
    }
    throw error;
  }
}

/** 是否作为入口脚本直接运行(而非被 release.ts 导入);用 path.relative 比较以兼容 Windows 盘符大小写差异 */
function isMainModule(): boolean {
  const entry = process.argv[1];
  return entry !== undefined && relative(resolve(entry), fileURLToPath(import.meta.url)) === "";
}

if (isMainModule()) {
  process.exit(runCli(process.argv.slice(2)));
}
