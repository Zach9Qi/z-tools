// 工具注册表:手工维护的模块数组,是启动器认识工具的唯一入口。
// 新工具只需在 modules 里追加一项;catalog 与 moduleOf 由此派生,不用另改。
import { clipboardTool } from "@/tools/clipboard";
import type { ToolItem, ToolModule } from "@/types/tool";

/** 全部已注册模块;数组顺序即主页「全部工具」分区的展示顺序 */
export const modules: ToolModule[] = [clipboardTool];

/** 供搜索 / 网格使用的条目列表,与 modules 一一对应 */
export const catalog: ToolItem[] = modules.map((module) => module.item);

/** id → 模块的索引;模块只有几十个,Map 主要是让 moduleOf 不必每次线性扫描 */
const modulesById = new Map<string, ToolModule>(modules.map((module) => [module.item.id, module]));

/**
 * 按条目找回其模块。
 * catalog 里的条目都来自 modules,正常流程不会找不到;找不到说明有人绕过 registry
 * 手工构造了 ToolItem,这是编程错误,直接抛出而不是静默返回 undefined。
 */
export function moduleOf(item: ToolItem): ToolModule {
  const module = modulesById.get(item.id);
  if (module === undefined) {
    throw new Error(`未注册的工具: ${item.id}`);
  }
  return module;
}
