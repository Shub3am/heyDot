# Hey Dot

A local-first voice and screen assistant for your Mac. Ask about what is on your screen and get a spoken answer, with models that run on your machine by default.

**Status:** being rebuilt from a hackathon prototype into a full app. The original prototype is preserved at tag [`v0-hackathon`](https://github.com/Shub3am/heyDot/tree/v0-hackathon).

## Planned for the first release

- Push-to-talk and typed questions about your screen, answered in a chat panel and spoken aloud
- Follow-up questions within a conversation
- Local models downloaded on first run, or your own cloud API key, clearly labelled when data leaves your Mac
- "Hey Dot" wake word, tools and MCP, document Q&A in later phases

## Develop

Requirements: macOS, Rust (via rustup; the pinned version installs itself), Node 26, pnpm 12.

    pnpm -C app install
    pnpm -C app tauri dev

See `AGENTS.md` for test and build commands.

## License

Apache-2.0. See `LICENSE`.
