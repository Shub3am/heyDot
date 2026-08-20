// The Settings page. Until the settings window exists it says what is coming, and keeps the one setting that works today.
// Must not offer a button for anything that does not work yet.

import { openScreenRecordingSettings } from "../chat/ipc";

export default function SettingsPage() {
  return (
    <section className="page">
      <header className="page-header">
        <h1>Settings</h1>
      </header>
      <div className="page-card">
        <h2>More settings</h2>
        <p>Hotkeys, voice, screenshot size, model choice and launch at login.</p>
        <p className="page-card-today">
          Hey Dot uses the local model shown in the sidebar. Screen Recording permission is managed by macOS.
        </p>
        <button className="secondary" onClick={() => void openScreenRecordingSettings()}>
          Open Screen Recording settings
        </button>
      </div>
    </section>
  );
}
