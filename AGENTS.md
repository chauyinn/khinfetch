# KhinFetch Development Guide

## Project Overview

- KhinFetch 是基于 Tauri 2、React、TypeScript 和 Rust 的跨平台桌面下载管理器。
- 前端使用 pnpm、Vite、Zustand、Radix UI、Lucide 和 CSS Modules；Rust 后端负责网络请求、HTML 解析、下载、SQLite、日志和文件系统操作。
- 前端通过 `src/api/client.ts` 中的 `BackendClient` 与后端通信，不在组件或 store 中直接调用 Tauri IPC。
- Rust 公共 DTO 位于 `src-tauri/src/domain/`，`src/api/generated/` 中的 TypeScript 类型必须与其保持一致。
- SQLite 和后端查询结果是任务状态的权威来源，Tauri 事件用于增量更新。
- 当前主要界面语言是简体中文。新增用户可见文本集中到 `src/i18n/`，并保持项目的非官方客户端定位和 GPL-3.0-or-later 许可。

## Coding Rules

- 功能改动请遵循最小变更以及最佳实践，尽量复用已有代码
- 涉及到大的架构或 API 变动，需要先讨论
- 删除功能或组件时请同步清理相关的死代码
