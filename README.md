# ForgeRust

A desktop application for technical interview practice, built with Tauri 2, Vue 3, and Rust.

## Features

- **Question Training** — Select questions by tag and difficulty, answer them one by one with a per-question timer. Supports single-choice, multi-choice, and open-ended question types.
- **AI Evaluation** — Answers are graded by an LLM (configurable API endpoint). The AI returns a score, standard answer, explanation, and comment.
- **Skip & Exit Safety** — Skip individual questions (counted as wrong in stats), and confirm before leaving a session mid-way.
- **Session Summary** — After each session, view score breakdown, time spent, and per-question AI feedback.
- **Wrong Question Book** — Questions answered incorrectly (or manually marked) are saved. Re-practice them directly from the wrong book.
- **AI Question Generation** — Generate new questions from a topic description using the configured LLM, then import them into the question bank.
- **CSV Import** — Bulk-import questions from CSV files. Tags are auto-assigned by the AI during import.
- **Settings** — Configure your own API key and base URL (compatible with OpenAI-format endpoints).

## Tech Stack

| Layer    | Technology                    |
|----------|-------------------------------|
| Frontend | Vue 3 + TypeScript + Vite     |
| Backend  | Rust (Tauri 2)                |
| Database | SQLite via SQLx               |
| AI       | HTTP calls to OpenAI-compatible LLM API |

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/)
- [Node.js](https://nodejs.org/) (v18+)
- [Tauri CLI prerequisites](https://v2.tauri.app/start/prerequisites/)

### Development

```bash
npm install
npm run tauri dev
```

### Build

```bash
npm run tauri build
```

## Configuration

On first launch, open **Settings** (gear icon) and enter your API key and base URL. The app defaults to `https://zenmux.ai/api/v1/chat/completions` but works with any OpenAI-compatible endpoint.

## Project Structure

```
src/                    # Vue frontend
  components/
    QuestionTraining.vue  # Core training flow
    WrongBook.vue         # Wrong question review
    AIGenerate.vue        # AI question generation
  App.vue               # Root layout + navigation

src-tauri/src/          # Rust backend
  lib.rs                # Tauri command handlers
  db.rs                 # SQLite queries
  llm_client.rs         # LLM API client
  models.rs             # Shared data models
```
