// invoke 封装层的汇出口:命令按领域拆在 `src/lib/api/<domain>.ts`,这里统一 re-export,
// 调用方既可 `import { hideLauncher } from "@/lib/api"`,也可按领域 `import ... from "@/lib/api/clipboard"`。
// 整个 src/ 只允许 `src/lib/api/**/*.ts` import `@tauri-apps/api/core`。
export * from "@/lib/api/clipboard";
export * from "@/lib/api/launcher";
