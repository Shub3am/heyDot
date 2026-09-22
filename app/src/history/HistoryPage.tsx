// The History page: turns saving on, then searches, reopens and deletes the saved chats.
// Must not show a chat's turns or call invoke directly; the Chat page shows a reopened chat, ../chat/ipc talks to Rust.

import { useEffect, useState } from "react";
import {
  deleteAllSavedChats,
  deleteSavedChat,
  historyStatus,
  listSavedChats,
  openSavedChat,
  setHistorySaving,
  turnOnHistory,
  type HistoryStatus,
  type SavedChatRow,
  type SavedTurnRow,
} from "../chat/ipc";
import "./history.css";

const SAVED_AT_FORMAT = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

function describeSize(turnCount: number) {
  return turnCount === 1 ? "1 question" : `${turnCount} questions`;
}

export default function HistoryPage({ onOpenChat }: { onOpenChat: (turns: SavedTurnRow[]) => void }) {
  const [status, setStatus] = useState<HistoryStatus | null>(null);
  const [search, setSearch] = useState("");
  const [chats, setChats] = useState<SavedChatRow[]>([]);
  const [listVersion, setListVersion] = useState(0);
  const [confirmingDeleteAll, setConfirmingDeleteAll] = useState(false);
  const [pageError, setPageError] = useState<string | null>(null);
  const isOn = status?.kind === "on";

  useEffect(() => {
    void historyStatus().then(setStatus);
  }, []);

  // A reply to an older search can land after a newer one, so only the latest request may set the list.
  useEffect(() => {
    if (!isOn) {
      return;
    }
    let isLatest = true;
    listSavedChats(search).then(
      (found) => isLatest && setChats(found),
      (error) => isLatest && setPageError(String(error)),
    );
    return () => {
      isLatest = false;
    };
  }, [isOn, search, listVersion]);

  // Every change is followed by a fresh list, so the page never shows a chat that is gone.
  async function changeHistory(change: () => Promise<unknown>) {
    setPageError(null);
    try {
      await change();
    } catch (error) {
      setPageError(String(error));
    }
    setListVersion((version) => version + 1);
  }

  const openChat = (chatId: number) => changeHistory(async () => onOpenChat(await openSavedChat(chatId)));
  const deleteChat = (chatId: number) => changeHistory(() => deleteSavedChat(chatId));
  const changeStatus = (nextStatus: () => Promise<HistoryStatus>) =>
    changeHistory(async () => setStatus(await nextStatus()));

  function deleteAllChats() {
    setConfirmingDeleteAll(false);
    void changeHistory(deleteAllSavedChats);
  }

  return (
    <section className="page">
      <header className="page-header">
        <h1>History</h1>
        {status?.kind === "on" && (
          <div className="history-actions">
            <button className="secondary" onClick={() => void changeStatus(() => setHistorySaving(!status.saving))}>
              {status.saving ? "Pause saving" : "Resume saving"}
            </button>
            {confirmingDeleteAll ? (
              <>
                <button className="destructive" onClick={deleteAllChats}>
                  Delete all chats
                </button>
                <button className="secondary" onClick={() => setConfirmingDeleteAll(false)}>
                  Cancel
                </button>
              </>
            ) : (
              <button className="secondary" onClick={() => setConfirmingDeleteAll(true)}>
                Delete all
              </button>
            )}
          </div>
        )}
      </header>
      {status?.kind === "off" && (
        <div className="page-card">
          <h2>Save your chats</h2>
          <p>Search, reopen and delete your chats later. They are stored encrypted on this Mac, never uploaded.</p>
          <p className="page-card-today">
            A chat lasts until you click New chat, stay idle for ten minutes, or quit Hey Dot.
          </p>
          <button onClick={() => void changeStatus(turnOnHistory)}>Turn on saving</button>
        </div>
      )}
      {status?.kind === "unavailable" && (
        <div className="page-card">
          <h2>Saved chats are unavailable</h2>
          <p role="alert">{status.reason}</p>
        </div>
      )}
      {status?.kind === "on" && (
        <div className="history-body">
          {!status.saving && (
            <p className="notice">Saving is paused. New chats are not saved until you resume.</p>
          )}
          <input
            className="history-search"
            type="search"
            aria-label="Search saved chats"
            placeholder="Search questions and answers"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
          />
          {pageError && (
            <p role="alert" className="error">
              {pageError}
            </p>
          )}
          {chats.length === 0 ? (
            <p className="history-empty">{search ? "No saved chats match your search." : "No saved chats yet."}</p>
          ) : (
            <ul className="history-list">
              {chats.map((chat) => (
                <li key={chat.id} className="history-row">
                  <div className="history-row-text">
                    <p className="history-title">{chat.title}</p>
                    <p className="history-meta">
                      {SAVED_AT_FORMAT.format(chat.updatedAtMs)} · {describeSize(chat.turnCount)}
                    </p>
                  </div>
                  <button className="secondary" onClick={() => void openChat(chat.id)}>
                    Open
                  </button>
                  <button className="secondary" onClick={() => void deleteChat(chat.id)}>
                    Delete
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </section>
  );
}
