# ForgeRust 产品体验升级设计文档

## 背景与目标

ForgeRust 当前已经具备题库训练、AI 出题、错题本、简历驱动模拟面试、面试复盘和面试记录等核心能力。下一阶段目标不是继续堆砌孤立功能，而是把应用从“功能可用”推进到“体验完整、面试感更真实、复盘能指导下一步学习”的阶段。

本次升级包含三条主线：

1. 在模拟面试回答区加入 Web Speech API 麦克风输入，降低口头表达转文字的成本。
2. 完成产品体验升级包：统一通知反馈、面试前配置、可行动复盘、题库质量控制。
3. 做一轮低风险代码调优：减少重复逻辑，拆出复用单元，为后续懒加载和更大功能扩展留下边界。

## 非目标

- 不接入云端语音转文字服务，第一版只使用浏览器/WebView 支持的 Web Speech API。
- 不做本地 Whisper/Vosk 离线识别，避免引入大模型依赖和复杂打包。
- 不重写现有业务架构，不做大规模 UI 换皮。
- 不把所有页面一次性重构为新组件体系，只改影响用户体验和维护成本最高的点。

## 设计原则

- 渐进增强：Web Speech API 不可用时，回答框仍保持完整手动输入能力。
- 反馈一致：成功、失败、警告和提示统一走 Toast，减少 `alert()` 打断式交互。
- 面试有目标：面试开始前让用户明确岗位、方向、难度、追问强度和面试模式。
- 复盘可行动：报告不仅展示分数，还能告诉用户接下来练什么。
- 质量可管理：AI 生成和导入题目要有来源与审核状态，避免题库长期失控。

## 功能设计

### 1. 麦克风输入

在 `MockInterview.vue` 的回答框操作区新增麦克风图标按钮。

状态包括：

- 不支持：按钮禁用，tooltip/提示文案说明当前 WebView 不支持语音输入。
- 空闲：点击开始识别。
- 识别中：按钮高亮，显示“正在听…”状态，再次点击停止。
- 出错：通过 Toast 提示权限、网络、无语音等错误。

识别结果追加到当前回答框文本末尾。用户仍需点击“回答”提交，避免误识别直接进入面试记录。

第一版语言固定为 `zh-CN`，连续识别关闭，使用短句输入模式，减少桌面端兼容风险。

### 2. 统一 Toast/通知

新增轻量通知系统：

- `src/composables/useToast.ts`：全局响应式 toast store。
- `src/components/ui/ToastHost.vue`：渲染通知队列。
- `AppShell.vue`：挂载 ToastHost。

通知类型：

- success：保存、导出、加入错题本等成功反馈。
- error：API 调用失败、导入失败、删除失败。
- warning：Web Speech API 不支持、麦克风权限被拒绝。
- info：普通提示。

本阶段优先替换关键页面现有 `alert()`：

- `QuestionLibrary.vue`
- `AIGenerate.vue`
- `QuestionTraining.vue`
- `Dashboard.vue`
- `MockInterview.vue`

### 3. 面试前配置产品化

在模拟面试的简历解析结果下方增加面试目标配置。

配置项：

- 目标岗位：默认“后端开发”，可手动输入。
- 岗位方向：Rust / Java / Go / 前端 / 数据库 / 通用基础，第一版用 select。
- 面试难度：easy / medium / hard。
- 追问强度：light / normal / deep。
- 面试模式：project_only / fundamental_only / full。

后端扩展 `mock_interviews` 表，持久化这些配置。`start_interview` 接收配置参数，`build_interviewer_system` 根据配置调整 system prompt。

模式行为：

- project_only：只进行项目环节，八股轮数置为 0。
- fundamental_only：跳过项目环节，从八股开始。
- full：项目 + 八股，沿用当前主流程。

### 4. 可行动复盘

扩展 `DimensionScores` 之外的复盘结构，新增：

- `weak_points: Vec<String>`：薄弱点。
- `recommended_tags: Vec<String>`：建议练习标签。
- `action_items: Vec<String>`：下一步行动建议。

后端 `evaluate_interview` 要求 LLM 返回这些字段；若解析失败，给出空数组和现有 fallback summary。

前端报告页和面试历史详情页展示：

- 薄弱点列表。
- 推荐练习标签。
- “去题库训练”按钮：把推荐标签作为训练入口参数传给 `QuestionTraining`。

第一版不新建训练计划表，只做轻量跳转，避免范围过大。

### 5. 题库质量控制

扩展 `questions` 表：

- `source TEXT NOT NULL DEFAULT 'manual'`
- `review_status TEXT NOT NULL DEFAULT 'approved'`
- `quality_note TEXT NOT NULL DEFAULT ''`

约定值：

- source：manual / ai_generated / imported
- review_status：draft / approved / needs_review

行为：

- 手动新增题默认 `manual + approved`。
- AI 出题保存默认 `ai_generated + needs_review`。
- 导入题默认 `imported + needs_review`。
- 题库管理列表显示来源和审核状态，并支持在编辑弹窗里修改审核状态和质量备注。
- 训练抽题默认仍包含全部题，避免旧用户突然找不到题；后续可再加“只练已审核题”过滤。

### 6. 代码调优

调优范围控制在低风险：

- 抽 `useSpeechRecognition.ts`，隔离 Web Speech API 兼容和状态管理。
- 抽 `interviewOptions.ts`，统一面试配置类型、默认值、显示文案。
- 抽 `interviewReport.ts` 或共享类型，减少 `MockInterview.vue` 与 `InterviewHistory.vue` 重复 interface。
- ECharts/pdfjs 暂不强制懒加载改造，先保留为后续优化点；若本阶段时间允许，再把 ECharts 图表封装为 `InterviewRadar.vue`。

## 数据流

语音输入：

`SpeechRecognition` 识别文本 -> `useSpeechRecognition` 暴露 transcript/error/listening -> `MockInterview.vue` 追加到 `answer` -> 用户提交 -> 现有 `interview_respond`。

面试配置：

前端配置表单 -> `start_interview` 参数 -> `mock_interviews` 持久化 -> `build_interviewer_system` 注入 prompt -> 面试报告和历史可回看配置摘要。

复盘行动项：

完整 transcript -> `evaluate_interview` -> JSON 解析为报告字段 -> `finish_interview2` 保存 JSON -> 报告页展示 -> 推荐标签跳转训练。

题库质量：

创建/导入/AI 保存题目 -> 写入 source/review_status/quality_note -> 题库列表与弹窗展示 -> 后续训练筛选可复用。

## 错误处理

- Web Speech API 不存在：Toast warning，不阻塞回答。
- 麦克风权限拒绝：Toast error，保留手动输入。
- 识别无结果：Toast info，用户可重试。
- LLM 复盘缺少新字段：使用空数组兜底，保留原 summary 与分数。
- 旧数据库无新增列：`init_db` 使用幂等 `ALTER TABLE` 迁移。

## 测试与验证

后端：

- 新增数据库迁移测试：旧库升级后 `questions` 与 `mock_interviews` 新字段可读写。
- 新增面试配置持久化测试。
- 新增复盘 JSON 解析容错测试。

前端：

- `npm run build` 确认类型与打包通过。
- 手动验证 Web Speech API 支持/不支持两种状态。
- 手动验证 Toast 替换关键 `alert()`。
- 手动验证 AI 生成题进入待审核状态，手动题为已审核。
- 手动验证面试报告展示薄弱点和推荐标签。

## 实施顺序

1. Toast 底座先落地，保证后续功能都有统一反馈。
2. 麦克风输入接入模拟面试。
3. 面试前配置和后端 prompt 接入。
4. 复盘报告扩展和历史页同步。
5. 题库质量字段与 UI 接入。
6. 做低风险代码调优和最终验证。
