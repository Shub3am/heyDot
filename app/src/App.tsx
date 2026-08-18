import ChatPanel from "./chat/ChatPanel";
import ModelStatus from "./localModel/ModelStatus";
import useLocalModelStatus from "./localModel/useLocalModelStatus";

function App() {
  const localModelStatus = useLocalModelStatus();
  return (
    <main className="app">
      <ModelStatus status={localModelStatus} />
      <ChatPanel status={localModelStatus} />
    </main>
  );
}

export default App;
