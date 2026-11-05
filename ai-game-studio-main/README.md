# Prompt Guide: Queen's Domain Lab

> A StarCraft-styled desktop tool for **vibe-coding games**. Answer a few questions about the game you want; it builds a step-by-step plan of ready-to-paste AI prompts, audits the AI's output with a model of your choice, and rates free local models against your actual PC.

*Queen's Domain*, a slow-paced retro ARPG blueprint (50 steps), ships as a built-in preset.

## Why

Vibe-coding a game fails when you ask an AI for "the whole game" at once. This tool breaks a game into small, ordered steps, writes each prompt for **your** engine, camera, genre and scope, then checks what the AI gives back against a rule set before you move on.

## Features

- **Project wizard**: 25 questions (platform, 2D/3D, camera, genre, sub-genre, art style, project size, multiplayer, devices, controls, story, difficulty, accessibility, budget, existing project, "feels like...") generate a custom plan.
- **Dynamic step count**: the same game is ~9 steps as a weekend jam and 30+ as a large project.
- **Platform-aware prompts**: engines get code instructions; Spawn.co and Rosebud get plain-language specs with no code.
- **Auditor**: paste the AI's answer, get a pass/fail verdict across 12 rules plus a ready-to-paste fix prompt.
- **Use the AI you already have**: Claude Code and ChatGPT Codex via their own sign-ins (no keys), plus Gemini, Kimi, OpenRouter, Ollama and LM Studio.
- **Free local models, rated for your PC**: a 4-level gauge (green, yellow, orange, red) says whether a model will run, with tuning tips, hover cards and a built-in benchmark.
- **"Can this PC build it?"**: the same gauge for engines (Unreal on a 4 GB GPU is flagged before you start).
- **Edit & export**: change a project's profile without losing progress; export the whole plan as one Markdown file for Claude Code or Cursor.

## Supported platforms

Spawn.co · Rosebud AI · Unity · Godot 4 · Unreal Engine 5 · Roblox Studio · GameMaker · Phaser 3 · Three.js/Babylon.js · Construct 3

Genres: Action RPG, RPG, FPS, Platformer, Roguelike, Strategy/RTS, Survival/Crafting, Puzzle, Racing, Simulation/Tycoon. Sub-genre twists from GameDiscoverCo's last public top-Steam-tags data (Open World Survival Craft, Colony Sim, Farming Sim, 4X, Base Building).

## Quick start (Windows)

Requirements: Windows 10/11, Node 18.15+, Microsoft Edge or Chrome. No `npm install` (zero dependencies).

```bash
npm run install-shortcut   # creates the "Queen's Domain" desktop icon
npm start                  # or run the helper directly, then open http://127.0.0.1:47831/
npm test                   # 17 dependency-free checks
```

Then: **Settings** → pick a model → **Load** (Claude/Codex: press **Log in** once) → **Projects** → **New Project**.

## How it works

```
 Edge app window ──HTTP──▶ server/server.js (127.0.0.1:47831) ──▶ Claude Code · Codex · Ollama · cloud APIs
 app/index.html + app/wizard.js            reads live hardware (RAM, GPU, disk)
```

`wizard.js` is pure data and functions: your answers become a *profile*, and `generate(profile)` returns the steps and prompts. The helper is a small zero-dependency Node server that detects installed tools, runs prompts, and reads hardware. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Repository layout

```
app/        Front end (index.html, wizard.js, icon.png)
server/     Local helper (server.js)
scripts/    install.ps1 (icon + shortcut), launcher.ps1
tests/      run.js
docs/       ARCHITECTURE · WIZARD · MODELS · TESTING · ROADMAP
AGENTS.md · CLAUDE.md · HANDOFF.md · CHANGELOG.md
```

## Documentation

[Architecture](docs/ARCHITECTURE.md) · [Wizard & generator](docs/WIZARD.md) · [Models & hardware ratings](docs/MODELS.md) · [Testing](docs/TESTING.md) · [Roadmap](docs/ROADMAP.md) · [Changelog](CHANGELOG.md)

## Status

Early (v0.1). Verified with automated checks and scripted UI runs. **Not yet verified on real use:** Ollama downloads/benchmarks, a full audit run with a real model, and the hover spinner animation. Hardware-requirement tables and local-model speeds are estimates until benchmarked. Details in [docs/ROADMAP.md](docs/ROADMAP.md).

## Privacy & security

Everything runs locally. The helper listens on 127.0.0.1 only and rejects other origins. API keys stay in the app's browser storage and are sent only to the provider you chose. Claude and Codex use their own sign-ins; the app never reads other apps' credential files.
