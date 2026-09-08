// 工具注册契约:启动器与各工具模块之间唯一的耦合点。
// 工具只需按此形状在 src/tools/registry.ts 登记即可出现在网格中;启动器不认识任何具体工具。
import type { Component } from "vue";

/** 启动器网格里的一个条目,是工具面向搜索 / 展示的元数据 */
export interface ToolItem {
  /** 唯一 id,registry 按此查模块 */
  id: string;
  /** 磁贴标题,同时参与搜索匹配 */
  title: string;
  /** lucide 图标名,经 tools/icons.ts 解析;未登记回退 puzzle */
  icon: string;
  /** 搜索时与 title 一起做小写子串匹配 */
  keywords: string[];
  /** 动态匹配:对搜索词返回 true 时进入「匹配结果」分区(如 URL / 路径) */
  accepts?: (query: string) => boolean;
  /** view = 激活后进入工具页;launch = 激活后直接执行 run() */
  action: "view" | "launch";
}

/** view 型工具:激活后在面板内挂载 page,搜索栏输入由 query prop 传入 */
export interface ViewToolModule {
  item: ToolItem & { action: "view" };
  /** 工具页组件,必须接收 prop `query: string` */
  page: Component;
  /** 工具页态搜索栏的占位文案;不给则由面板用默认值 */
  placeholder?: string;
  // `never` 让两种模块互斥:写了 run 的对象无法被推断为 view 型
  run?: never;
}

/** launch 型工具:激活后执行一次 run(),不进入工具页 */
export interface LaunchToolModule {
  item: ToolItem & { action: "launch" };
  /** 失败时应 reject,由面板统一记录日志并展示错误文案 */
  run: (ctx: { query: string }) => Promise<void>;
  page?: never;
  placeholder?: never;
}

export type ToolModule = ViewToolModule | LaunchToolModule;

/**
 * 是否为 view 型模块。
 * 以 `item.action` 为判别字段(而非是否存在 page),与 ToolItem 上的声明保持单一真相。
 */
export function isViewModule(module: ToolModule): module is ViewToolModule {
  return module.item.action === "view";
}
