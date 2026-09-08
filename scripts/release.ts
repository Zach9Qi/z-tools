/**
 * 一键发布脚本:安全检查 → 同步版本号 → commit → tag → push。
 *
 * 用法(经 package.json 的 release 脚本):
 *   bun run release <x.y.z | patch | minor | major> [--dry-run] [--no-push]
 *
 * - 安全检查全部通过才会动文件:工作区干净、位于 main、与 origin/main 同步、tag 不存在
 * - 版本号写入 package.json / Cargo.toml / tauri.conf.json5 并刷新 Cargo.lock(见 version.ts)
 * - `--dry-run`:只读的 git 查询照常执行(让检查结果真实),所有写文件 / git 写操作改为打印计划
 * - `--no-push`:本地 commit + tag 后停下,由用户自行 push
 * - push 之后由 .github/workflows/release.yml 接手跨平台打包并创建 GitHub Release
 *
 * git 一律通过 execFileSync 传数组参数,不拼 shell 字符串;只用 Node 内置模块。
 */
import { execFileSync } from "node:child_process";
import {
  BUMP_KINDS,
  bumpVersion,
  CARGO_LOCK,
  describeError,
  isBumpKind,
  isValidVersion,
  readVersions,
  refreshCargoLock,
  REPO_ROOT,
  VERSION_FILES,
  VersionError,
  writeVersions,
} from "./version";

/** 发布流程被拒绝或参数错误;message 是面向用户的中文说明 */
export class ReleaseError extends Error {}

/** 命令行参数解析结果 */
export interface ReleaseOptions {
  /** 目标版本:具体 semver(可带 v 前缀)或递增类型 patch / minor / major */
  spec: string;
  /** 只打印计划,不写文件、不执行 git 写操作 */
  dryRun: boolean;
  /** 本地 commit + tag 后停下,不 push */
  noPush: boolean;
}

/** 发布时受控的分支与远端;脚本只在这条链路上工作 */
const RELEASE_BRANCH = "main";
const REMOTE = "origin";

/** 版本提交只允许包含这四个文件,避免把工作区里其他改动一起带进发布提交 */
const RELEASE_FILES: readonly string[] = [
  VERSION_FILES.packageJson,
  VERSION_FILES.cargoToml,
  VERSION_FILES.tauriConf,
  CARGO_LOCK,
];

const USAGE = `用法:bun run release <x.y.z | ${BUMP_KINDS.join(" | ")}> [--dry-run] [--no-push]`;

/** 解析命令行参数;格式不对抛 ReleaseError */
export function parseArgs(argv: string[]): ReleaseOptions {
  let spec: string | undefined;
  let dryRun = false;
  let noPush = false;
  for (const arg of argv) {
    if (arg === "--dry-run") {
      dryRun = true;
    } else if (arg === "--no-push") {
      noPush = true;
    } else if (arg.startsWith("-")) {
      throw new ReleaseError(`未知参数 ${arg}\n${USAGE}`);
    } else if (spec === undefined) {
      spec = arg;
    } else {
      throw new ReleaseError(`多余的参数 ${arg}\n${USAGE}`);
    }
  }
  if (spec === undefined) {
    throw new ReleaseError(`缺少目标版本\n${USAGE}`);
  }
  return { spec, dryRun, noPush };
}

/** 把 spec(具体版本或递增类型)解析成目标版本号;current 为 package.json 当前版本 */
export function resolveTargetVersion(spec: string, current: string): string {
  if (isBumpKind(spec)) {
    return bumpVersion(current, spec);
  }
  const version = spec.startsWith("v") ? spec.slice(1) : spec;
  if (!isValidVersion(version)) {
    throw new ReleaseError(
      `目标版本 ${spec} 无效:请传 x.y.z(可带 -预发布后缀)或 ${BUMP_KINDS.join(" / ")}`,
    );
  }
  return version;
}

/** 执行只读 git 查询并返回去掉末尾换行的 stdout(保留 status --porcelain 的首列空格);失败抛出原始错误 */
function gitQuery(args: string[]): string {
  return execFileSync("git", args, {
    cwd: REPO_ROOT,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trimEnd();
}

/** 执行会改动仓库的 git 命令;输出直通终端让用户看到 git 自己的提示,失败转成 ReleaseError */
function gitRun(args: string[]): void {
  try {
    execFileSync("git", args, { cwd: REPO_ROOT, stdio: "inherit" });
  } catch (error) {
    throw new ReleaseError(`git ${args.join(" ")} 执行失败:${describeError(error)}`);
  }
}

/**
 * 发布前安全检查,返回全部未通过项的中文说明(空数组表示通过)。
 * 只做只读查询(git fetch 只更新远端跟踪分支,不动工作区),dry-run 下也照常执行。
 */
export function runSafetyChecks(tag: string): string[] {
  const problems: string[] = [];

  const dirty = gitQuery(["status", "--porcelain"]);
  if (dirty) {
    const lines = dirty.split("\n");
    const preview = lines.slice(0, 10).join("\n    ");
    const more = lines.length > 10 ? `\n    ……共 ${lines.length} 项` : "";
    problems.push(`工作区有未提交改动(含未跟踪文件),请先提交或 stash:\n    ${preview}${more}`);
  }

  const branch = gitQuery(["branch", "--show-current"]);
  if (branch !== RELEASE_BRANCH) {
    problems.push(`请在 ${RELEASE_BRANCH} 分支发布(当前:${branch || "detached HEAD"})`);
  }

  try {
    gitQuery(["fetch", "--quiet", REMOTE, RELEASE_BRANCH]);
    const local = gitQuery(["rev-parse", "HEAD"]);
    const remote = gitQuery(["rev-parse", `${REMOTE}/${RELEASE_BRANCH}`]);
    if (local !== remote) {
      problems.push(
        `本地与远端不同步,请先 pull / push(本地 ${local.slice(0, 7)},${REMOTE}/${RELEASE_BRANCH} ${remote.slice(0, 7)})`,
      );
    }
  } catch (error) {
    problems.push(`无法从远端 ${REMOTE} 获取 ${RELEASE_BRANCH}:${describeError(error)}`);
  }

  if (gitQuery(["tag", "--list", tag])) {
    problems.push(`tag ${tag} 已存在(本地)`);
  } else {
    try {
      if (gitQuery(["ls-remote", "--tags", REMOTE, `refs/tags/${tag}`])) {
        problems.push(`tag ${tag} 已存在(远端 ${REMOTE})`);
      }
    } catch (error) {
      problems.push(`无法查询远端 ${REMOTE} 的 tag:${describeError(error)}`);
    }
  }

  return problems;
}

/** 从 origin 的 URL 推出 GitHub Actions 页面地址;不是 GitHub 仓库时返回 undefined */
export function actionsUrlFromRemote(remoteUrl: string): string | undefined {
  const match = /github\.com[:/]([^/]+)\/([^/]+?)(?:\.git)?$/.exec(remoteUrl.trim());
  return match ? `https://github.com/${match[1]}/${match[2]}/actions` : undefined;
}

/** 打印一条计划 / 执行日志;dry-run 下统一带「[dry-run] 将执行」前缀 */
function step(dryRun: boolean, message: string): void {
  console.log(dryRun ? `[dry-run] 将执行:${message}` : `→ ${message}`);
}

/** 主流程;返回进程退出码 */
export function main(argv: string[]): number {
  const options = parseArgs(argv);
  const { dryRun, noPush } = options;

  const current = readVersions();
  const version = resolveTargetVersion(options.spec, current.packageJson);
  const tag = `v${version}`;
  const commitMessage = `chore(release): ${tag}`;

  console.log(
    `发布 ${tag}(当前 ${current.packageJson})${dryRun ? " —— dry-run,不会改动任何文件" : ""}`,
  );

  console.log("安全检查:");
  const problems = runSafetyChecks(tag);
  if (problems.length === 0) {
    console.log(
      `  [通过] 工作区干净 / 位于 ${RELEASE_BRANCH} / 与 ${REMOTE}/${RELEASE_BRANCH} 同步 / tag ${tag} 不存在`,
    );
  } else {
    for (const problem of problems) {
      console.error(`  [失败] ${problem}`);
    }
    if (!dryRun) {
      console.error("安全检查未通过,已中止,未做任何改动。");
      return 1;
    }
  }

  // 三处已是目标版本时(例如手动改过版本号只差打 tag)不再制造空提交,直接打 tag
  const alreadyBumped =
    current.packageJson === version &&
    current.cargoToml === version &&
    current.tauriConf === version &&
    current.cargoLock === version;

  if (alreadyBumped) {
    console.log(`  版本号已全部为 ${version},跳过写文件与 commit,仅打 tag`);
  } else {
    step(dryRun, `写入版本号 ${version} → ${Object.values(VERSION_FILES).join(", ")}`);
    step(dryRun, `刷新 ${CARGO_LOCK}(cargo update --workspace --offline)`);
    if (!dryRun) {
      writeVersions(version);
      refreshCargoLock();
    }

    step(dryRun, `git add -- ${RELEASE_FILES.join(" ")}`);
    step(dryRun, `git commit -m "${commitMessage}"`);
    if (!dryRun) {
      gitRun(["add", "--", ...RELEASE_FILES]);
      gitRun(["commit", "--quiet", "-m", commitMessage]);
    }
  }

  // 用附注 tag:`git push --follow-tags` 只会带上附注 tag,轻量 tag 会被落下
  step(dryRun, `git tag -a ${tag} -m "${tag}"`);
  if (!dryRun) {
    gitRun(["tag", "-a", tag, "-m", tag]);
  }

  const pushCommand = `git push --follow-tags ${REMOTE} ${RELEASE_BRANCH}`;
  if (noPush) {
    console.log(`已指定 --no-push,跳过推送;确认无误后手动执行:${pushCommand}`);
  } else {
    step(dryRun, pushCommand);
    if (!dryRun) {
      gitRun(["push", "--follow-tags", REMOTE, RELEASE_BRANCH]);
    }
  }

  let actionsUrl: string | undefined;
  try {
    actionsUrl = actionsUrlFromRemote(gitQuery(["remote", "get-url", REMOTE]));
  } catch {
    actionsUrl = undefined;
  }
  if (actionsUrl) {
    console.log(`推送 tag 后 GitHub Actions 会自动打包并创建 Release,进度见:${actionsUrl}`);
  }

  if (dryRun && problems.length > 0) {
    console.error(`dry-run 结束:有 ${problems.length} 项安全检查未通过,实际执行会被拒绝`);
    return 1;
  }
  console.log(dryRun ? "dry-run 结束,未改动任何文件" : `发布 ${tag} 完成`);
  return 0;
}

try {
  process.exit(main(process.argv.slice(2)));
} catch (error) {
  if (error instanceof ReleaseError || error instanceof VersionError) {
    console.error(`发布中止:${error.message}`);
    process.exit(1);
  }
  throw error;
}
