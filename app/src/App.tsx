import { useState } from "react";
import ChatPanel from "./chat/ChatPanel";
import { newChat } from "./chat/ipc";
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
  const [chatKey, setChatKey] = useState(0);
  const isChatOpen = currentPage === "chat";

  // A new key remounts the Chat page with no turns, and newChat tells the backend to forget the conversation.
  function startNewChat() {
    setCurrentPage("chat");
    setChatKey((key) => key + 1);
    void newChat();
  }

  return (
    <div className="app">
      <Sidebar currentPage={currentPage} onOpenPage={setCurrentPage} onNewChat={startNewChat}>
        <ModelStatus status={localModelStatus} />
      </Sidebar>
      <main className="page-area">
        {/* Chat stays mounted on other pages so a running answer and its turns survive the switch. */}
        <div className="page-slot" hidden={!isChatOpen}>
          <ChatPanel key={chatKey} status={localModelStatus} isVisible={isChatOpen} />
        </div>
        {currentPage === "history" && <HistoryPage />}
        {currentPage === "documents" && <DocumentsPage />}
        {currentPage === "settings" && <SettingsPage />}
      </main>
    </div>
  );
}

export default App;
