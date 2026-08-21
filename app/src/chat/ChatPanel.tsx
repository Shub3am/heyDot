// The first chat surface: a conversation with the local model, typed one question at a time.
// Must not call invoke directly; every backend call goes through ./ipc.

import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";
import ChatTurn, { type Turn } from "./ChatTurn";
import {
  askText,
  downloadLocalModel,
  newChat,
  watchLocalModel,
  type LocalModelPhase,
  type LocalModelStatus,
} from "./ipc";

const SERVER_LOG_PATH = "~/Library/Logs/Hey Dot/llama-server.log";

const STATUS_DOT: Record<LocalModelPhase["kind"], "ready" | "busy" | "failed"> = {
  notInstalled: "failed",
  downloading: "busy",
  downloadFailed: "failed",
  starting: "busy",
  ready: "ready",
  down: "failed",
};

function describeModel(status: LocalModelStatus | null) {
  if (status === null) {
    return <p>Checking the local model...</p>;
  }
  const { modelName, phase } = status;
  switch (phase.kind) {
    case "notInstalled":
      return (
        <p>
          {modelName} is not downloaded yet ({(status.downloadBytes / 1e9).toFixed(1)} GB).{" "}
          <button onClick={() => void downloadLocalModel()}>Download {modelName}</button>
        </p>
      );
    case "downloading":
      return (
        <p>
          Downloading {modelName}: {phase.percent}%
        </p>
      );
    case "downloadFailed":
      return (
        <p>
          The download failed: {phase.reason}{" "}
          <button onClick={() => void downloadLocalModel()}>Try again</button>
        </p>
      );
    case "starting":
      return <p>Starting {modelName}...</p>;
    case "ready":
      return <p>{modelName} is ready.</p>;
    case "down":
      return (
        <p>
          {modelName} stopped: {phase.reason}. Details are in {SERVER_LOG_PATH}
        </p>
      );
  }
}

export default function ChatPanel() {
  const [status, setStatus] = useState<LocalModelStatus | null>(null);
  const [question, setQuestion] = useState("");
  const [turns, setTurns] = useState<Turn[]>([]);
  const nextTurnId = useRef(0);
  const conversation = useRef<HTMLDivElement>(null);
  const wasScrolledToBottom = useRef(true);

  useEffect(() => {
    void watchLocalModel(setStatus);
  }, []);

  // Follows a streaming answer only while the user has not scrolled up to read an earlier one.
  useLayoutEffect(() => {
    const element = conversation.current!;
    if (wasScrolledToBottom.current) {
      element.scrollTop = element.scrollHeight;
    }
  }, [turns]);

  function rememberScrollPosition() {
    const element = conversation.current!;
    wasScrolledToBottom.current = element.scrollHeight - element.scrollTop - element.clientHeight < 40;
  }

  // A turn removed by New chat is not found, so its late events change nothing.
  function updateTurn(id: number, update: (turn: Turn) => Turn) {
    setTurns((current) => current.map((turn) => (turn.id === id ? update(turn) : turn)));
  }

  async function ask() {
    const id = nextTurnId.current++;
    setQuestion("");
    setTurns((current) => [...current, { id, question, text: "", badge: null, screen: null, error: null }]);
    try {
      await askText(question, (event) => {
        if (event.event === "started") {
          const { leavesDevice, host, screen, forgotEarlierTurns } = event.data;
          if (forgotEarlierTurns) {
            setTurns((current) => current.filter((turn) => turn.id >= id));
          }
          updateTurn(id, (turn) => ({ ...turn, badge: leavesDevice ? `Sent to ${host}` : "On this Mac", screen }));
        } else {
          updateTurn(id, (turn) => ({ ...turn, text: turn.text + event.data.text }));
        }
      });
    } catch (error) {
      updateTurn(id, (turn) => ({ ...turn, error: String(error) }));
    }
  }

  function startNewChat() {
    setTurns([]);
    void newChat();
  }

  const canAsk = status?.phase.kind === "ready" && question.trim() !== "";

  // Enter while an input method is composing picks a candidate (Japanese, Chinese), so it must not ask.
  function askOnEnter(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key !== "Enter" || event.shiftKey || event.nativeEvent.isComposing) {
      return;
    }
    event.preventDefault();
    if (canAsk) {
      void ask();
    }
  }

  return (
    <section className="chat">
      <header className="chat-header">
        <div className="model-status" data-dot={status ? STATUS_DOT[status.phase.kind] : "busy"}>
          <h1>Hey Dot</h1>
          {describeModel(status)}
        </div>
        <button className="secondary" onClick={startNewChat}>
          New chat
        </button>
      </header>
      <div className="conversation" ref={conversation} onScroll={rememberScrollPosition}>
        {turns.length === 0 && <p className="empty">Ask about anything on your screen.</p>}
        {turns.map((turn) => (
          <ChatTurn key={turn.id} turn={turn} />
        ))}
      </div>
      <footer className="composer">
        <textarea
          aria-label="Question"
          placeholder="Ask about your screen..."
          rows={1}
          value={question}
          onChange={(event) => setQuestion(event.target.value)}
          onKeyDown={askOnEnter}
        />
        <button className="send" aria-label="Ask" disabled={!canAsk} onClick={() => void ask()}>
          <span aria-hidden="true">↑</span>
        </button>
      </footer>
    </section>
  );
}
