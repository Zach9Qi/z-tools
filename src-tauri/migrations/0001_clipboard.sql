-- 【不可变】本文件一旦提交不得再修改（包括注释、空白）：sqlx 按整个文件的 checksum 校验已应用的迁移，
-- 任何改动都会让已有用户的库报 "migration 1 was previously applied but has been modified" 而被判为损坏。
-- 需要变更 schema 一律新增 000N_xxx.sql。
--
-- 剪贴板历史表（sqlx::migrate! 编译期内嵌，运行时首次连接执行）。
-- 列语义详见 .trellis/tasks/09-09-clipboard-tool/design.md §3.2 与 src/clipboard.rs 模块文档。
CREATE TABLE clipboard_items (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  kind          TEXT    NOT NULL CHECK (kind IN ('text','image','files')),
  hash          TEXT    NOT NULL UNIQUE,   -- blake3 hex，均以类型域 "<kind>\0" 为前缀防跨类型碰撞；text=blake3("text\0",utf8)；
                                           -- image=blake3("image\0",w,h,rgba8)；files=blake3("files\0",paths.join("\n"))（见 clipboard.rs::domain_hash）
  text          TEXT,                      -- 可搜索 / 可展示的文本，含义随 kind：text=全文；files=各文件名（含扩展名）以 "\n" 拼接；image=NULL（无名，将来 OCR 文字填此列）
  image_file    TEXT,                      -- kind=image：images/ 下文件名 <hash>.png（缩略图 <hash>.thumb.png 同名推导）
  image_width   INTEGER,
  image_height  INTEGER,
  files         TEXT,                      -- kind=files：完整路径 JSON string[]，用于粘贴写回与 missing 检查，不参与搜索
  size          INTEGER NOT NULL,          -- text: utf8 字节；image: png 字节；files: 0
  favorite      INTEGER NOT NULL DEFAULT 0, -- 收藏标记：只是筛选维度 + 保护不被淘汰 / 清空，不影响排序
  created_at    INTEGER NOT NULL,          -- unix ms 首次
  copied_at     INTEGER NOT NULL           -- unix ms 最近一次；主排序键，并列时由 id 兜底
);
-- 不建额外索引：表最大约 500 非收藏 + 收藏，千行量级全扫 + 排序是微秒级，且列表查询几乎都带 LIKE（必然全扫）。
-- 去重所需的索引由 hash UNIQUE 隐式提供。MAX_ITEMS 提到万级时再加 (copied_at DESC, id DESC)。
