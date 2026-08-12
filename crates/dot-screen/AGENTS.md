# dot-screen

Owns: capturing the display under the mouse cursor as a JPEG for a vision model, with Hey Dot's own windows left out, scaled so its long edge fits the caller's limit, and telling a missing Screen Recording permission apart from every other failure.

Must not know about: models, providers, the settings file or the UI. The caller passes the size limit and decides what to do without an image.

Entry points: `capture_display_under_cursor(max_long_edge_px)`, blocking, returns JPEG bytes or `ScreenError`; `request_screen_access()`.

Invariants and gotchas:
- Never upscales: a display smaller than the limit is captured at its native pixel size. Aspect is always kept.

Called by: `app/src-tauri` (the `ask_text` and `open_screen_recording_settings` commands).
