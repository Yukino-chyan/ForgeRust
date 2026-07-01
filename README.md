# ForgeRust

ForgeRust 是一款面向程序员面试备考的桌面端智能训练软件，使用 Tauri 2、Vue 3、TypeScript、Rust 和 SQLite 构建。

它的目标不是单纯做一个刷题工具，而是把题库训练、AI 出题、模拟面试、错题整理、面试复盘和长期训练记录整合成一个完整的个人备考闭环。

## 主要功能

- **题库训练**：按考点/标签选择题目进行练习，支持单选题、多选题和简答题。
- **AI 点评**：简答题可调用大模型进行评分，返回标准答案、解析、分数和点评。
- **题库管理**：支持浏览、搜索、新增、编辑、导入、导出题目，并维护题目标签。
- **AI 出题**：按考点、题型、难度和补充要求生成题目，并一键加入本地题库。
- **错题本**：自动沉淀答错题目，也支持手动加入错题，便于后续重练。
- **模拟面试**：支持简历解析、项目追问、八股追问和多轮 AI 面试对话。
- **面试复盘**：面试后生成维度评分、薄弱点、推荐重练标签和下一步行动建议。
- **语音输入**：模拟面试回答框支持麦克风语音转文字；如果当前系统环境不支持，也可以手动输入。
- **本地存储**：题库、训练记录、错题、简历摘要和面试历史默认保存在本地 SQLite 数据库中。

## 普通用户如何使用

如果你只是想使用软件，推荐使用已经打包好的安装包；如果拿到的是源码仓库，则需要先按下面说明自行构建安装包。

### 1. 获取或构建安装包

如果你拿到的是课程提交包或发布包，优先使用已经打包好的 Windows 安装包：

```text
release-package/forgerust_0.1.0_x64-setup.exe
```

> `release-package/` 用于本地交付和答辩展示，通常不提交到 Git 仓库。GitHub 使用者可以从 Release 下载安装包，或按源码构建步骤自行打包。

双击该安装包，按照提示完成安装即可。

如果你拿到的是 GitHub 源码仓库，仓库中通常不会提交安装包二进制文件。请先安装开发环境，然后在项目根目录执行：

```powershell
npm install
npm run tauri build
```

打包完成后，安装包通常位于：

```text
src-tauri/target/release/bundle/nsis/forgerust_0.1.0_x64-setup.exe
```

也可以直接运行 release 版可执行文件：

```text
src-tauri/target/release/forgerust.exe
```

备用 MSI 安装包通常位于：

```text
src-tauri/target/release/bundle/msi/forgerust_0.1.0_x64_en-US.msi
```

如果本机设置了 `CARGO_TARGET_DIR` 或 `.cargo/config.toml` 自定义构建目录，请以 `npm run tauri build` 结束时输出的实际路径为准。

### 2. 首次启动

安装完成后，像普通桌面软件一样从开始菜单或安装目录打开 `forgerust`。

首次打开后建议先进入左下角 **设置** 页面，填写 AI 服务配置：

- **API Key**：你的大模型 API 密钥。
- **API URL**：兼容 OpenAI `chat/completions` 格式的接口地址。
- **模型名称**：例如 `deepseek-chat`。

项目默认 API URL 为：

```text
https://zenmux.ai/api/v1/chat/completions
```

只要接口兼容 OpenAI 的聊天补全格式，也可以替换成其他服务商地址。

> 如果不配置 API Key，题库浏览、题库管理、本地训练记录等基础功能仍可使用；AI 点评、AI 出题、简历解析、模拟面试和复盘等功能需要联网并配置可用 API。

### 3. 开始练习

推荐使用顺序：

1. 进入 **题库管理**，查看已有题目，或导入自己的题库。
2. 进入 **题库训练**，选择考点和题量，开始答题。
3. 答错的题目会进入 **错题本**，后续可以集中复习。
4. 如果某个知识点薄弱，可以进入 **AI 出题** 生成补充题目。
5. 进入 **模拟面试**，上传或粘贴简历内容，设置岗位方向、难度和追问强度，开始模拟面试。
6. 面试结束后查看 **面试记录** 和复盘建议。

## 题库导入格式

题库导入使用 JSON 文件。文件内容应是一个数组，每个元素表示一道题。

字段说明：

| 字段 | 是否必填 | 说明 |
| --- | --- | --- |
| `question_type` | 是 | 题型，取值为 `SINGLE`、`MULTI`、`ESSAY` |
| `content` | 是 | 题干 |
| `options` | 否 | 选择题选项数组；简答题可以为空或省略 |
| `tags` | 是 | 标签，可用逗号分隔，例如 `计算机网络,TCP` |
| `difficulty` | 是 | 难度，建议 1-5 |
| `standard_answer` | 否 | 标准答案 |
| `explanation` | 否 | 题目解析 |
| `source` | 否 | 题目来源，例如 `手动整理`、`AI 生成`、`导入题库` |
| `quality_status` | 否 | 质量状态，例如 `unchecked`、`approved`、`needs_review` |
| `quality_note` | 否 | 质量备注，例如来源说明、重复题线索等 |

示例：

```json
[
  {
    "question_type": "SINGLE",
    "content": "TCP 三次握手的主要目的是什么？",
    "options": [
      "A. 确认双方收发能力正常并同步初始序列号",
      "B. 加密传输内容",
      "C. 压缩网络数据",
      "D. 关闭连接"
    ],
    "tags": "计算机网络,TCP",
    "difficulty": 2,
    "standard_answer": "A",
    "explanation": "TCP 三次握手用于确认通信双方的发送和接收能力，并同步初始序列号。",
    "source": "示例题库",
    "quality_status": "unchecked",
    "quality_note": "可用于基础网络面试训练"
  },
  {
    "question_type": "ESSAY",
    "content": "请简述 Rust 所有权机制解决了什么问题。",
    "options": [],
    "tags": "Rust,所有权",
    "difficulty": 3,
    "standard_answer": "Rust 通过所有权、借用和生命周期在编译期管理内存安全，减少悬垂指针和数据竞争等问题。",
    "explanation": "回答时应说明所有权转移、不可变/可变借用以及编译期检查的意义。",
    "source": "示例题库"
  }
]
```

## 数据保存位置

应用会把配置和数据保存在本机用户目录下。Windows 环境中通常位于：

```text
%APPDATA%/com.asus.forgerust/
```

其中常见文件包括：

- `config.json`：API Key、API URL、模型名称等配置。
- `forgerust.db`：本地 SQLite 数据库，保存题库、训练记录、错题、简历和面试历史。

如果要迁移数据，可以备份该目录下的数据库文件和配置文件。

## 开发者如何从源码运行

如果你想继续开发或修改源码，需要先准备开发环境。

### 1. 环境要求

- Node.js 18 或更高版本。
- Rust 稳定版工具链。
- Windows WebView2 Runtime；Windows 11 通常已自带，Windows 10 如缺失需要单独安装。
- Tauri 2 所需的 Windows 构建工具。

### 2. 安装依赖

在项目根目录执行：

```powershell
npm install
```

### 3. 开发模式启动

```powershell
npm run tauri dev
```

注意：这是开发调试模式，每次启动可能会触发前端开发服务器和 Rust 编译检查，因此启动会比普通桌面应用慢。

### 4. 正式打包

```powershell
npm run tauri build
```

打包完成后会生成 release 可执行文件和安装包。默认情况下常见产物包括：

```text
src-tauri/target/release/forgerust.exe
src-tauri/target/release/bundle/nsis/forgerust_0.1.0_x64-setup.exe
src-tauri/target/release/bundle/msi/forgerust_0.1.0_x64_en-US.msi
```

如果本机设置了 `CARGO_TARGET_DIR` 或 `.cargo/config.toml` 自定义构建目录，请以构建日志输出的实际路径为准。

答辩展示或交付给他人使用时，建议提供 `forgerust_0.1.0_x64-setup.exe`。

## 技术栈

| 层级 | 技术 |
| --- | --- |
| 前端 | Vue 3 + TypeScript + Vite |
| 桌面端 | Tauri 2 |
| 后端逻辑 | Rust |
| 数据库 | SQLite + SQLx |
| AI 能力 | OpenAI-compatible HTTP API |
| 图表 | ECharts |
| 图标 | lucide-vue-next |
| PDF 文本提取 | pdfjs-dist |

## 项目结构

```text
src/                         Vue 前端源码
  components/                页面组件和 UI 组件
  composables/                前端组合式逻辑，例如 toast、语音输入、导入进度
  styles/                     全局样式与主题变量
  utils/                      PDF 文本提取等工具

src-tauri/                   Tauri / Rust 后端
  src/config.rs              API 配置读写
  src/db.rs                  SQLite 初始化与查询
  src/llm_client.rs          大模型调用、出题、评分、简历解析和复盘
  src/lib.rs                 Tauri command 与核心业务流程
  src/models.rs              前后端共享的数据结构

release-package/             本地发布包复制目录；用于课程提交或答辩展示，默认不提交仓库
```

## 常见问题

### 为什么运行 `npm run tauri dev` 每次都像在编译？

因为 `tauri dev` 是开发模式，会启动前端开发服务器并检查/编译 Rust 代码。普通用户不需要使用这个命令，直接运行打包后的安装包或 release 版 exe 即可。

### 为什么 AI 功能报 API Key 未配置？

请进入左下角 **设置** 页面，填写 API Key、API URL 和模型名称。填写后保存，再重新使用 AI 出题、AI 点评或模拟面试功能。

### 为什么网络请求失败？

通常是以下原因之一：

- API Key 错误或额度不足。
- API URL 填写错误。
- 当前网络无法访问对应模型服务。
- 代理软件未开启或规则不正确。

### 为什么语音输入不可用？

语音输入依赖当前系统 WebView 环境对 Web Speech API 的支持。如果不可用，可以直接手动输入回答，不影响模拟面试主流程。

## 项目定位

ForgeRust 更像一个个人面试备考工作台，而不是公共刷题网站。它的核心优势是：

- 题库可以长期维护。
- 训练记录可以沉淀。
- AI 可以辅助出题、追问和复盘。
- 简历和项目经历可以进入模拟面试流程。
- 数据默认保存在本地，隐私和可控性更强。
