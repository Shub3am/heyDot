import { useState } from "react";
import ChatPanel from "./chat/ChatPanel";
import { newChat, type SavedTurnRow } from "./chat/ipc";
import DocumentsPage from "./documents/DocumentsPage";
import HistoryPage from "./history/HistoryPage";
import ModelStatus from "./localModel/ModelStatus";
import useLocalModelStatus from "./localModel/useLocalModelStatus";
import SettingsPage from "./settings/SettingsPage";
import type { Page } from "./shell/pages";
import Sidebar from "./shell/Sidebar";

function App() {
  const localModelStatus = useLocalModelStatus();
  const [currentPage, setCurrentPage] = useState<Page>("chat");
  const [shownChat, setShownChat] = useState({ key: 0, savedTurns: [] as SavedTurnRow[] });

  // A new key remounts the Chat page, which reads savedTurns only when it mounts.
  function showChat(savedTurns: SavedTurnRow[]) {
    setCurrentPage("chat");
    setShownChat((chat) => ({ key: chat.key + 1, savedTurns }));
  }

  // newChat tells the backend to forget the conversation; open_saved_chat already resumed a saved one.
  function startNewChat() {
    showChat([]);
    void newChat();
  }

  return (
    <div className="app">
      <Sidebar currentPage={currentPage} onOpenPage={setCurrentPage} onNewChat={startNewChat}>
        <ModelStatus status={localModelStatus} />
      </Sidebar>
      <main className="page-area">
        {/* Chat stays mounted on other pages so a running answer and its turns survive the switch. */}
        <div className="page-slot" hidden={currentPage !== "chat"}>
          <ChatPanel key={shownChat.key} status={localModelStatus} savedTurns={shownChat.savedTurns} />
        </div>
        {currentPage === "history" && <HistoryPage onOpenChat={showChat} />}
        {currentPage === "documents" && <DocumentsPage />}
        {currentPage === "settings" && <SettingsPage />}
      </main>
    </div>
  );
}

export default App;
