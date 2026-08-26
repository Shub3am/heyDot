// The first chat surface: a conversation with the local model, typed one question at a time.
// Must not call invoke directly; every backend call goes through ./ipc.

import { useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";
import ChatTurn, { type Turn } from "./ChatTurn";
import { askText, newChat, type LocalModelStatus } from "./ipc";

/** `status` only decides whether a question can be asked; the sidebar shows it. */
export default function ChatPanel({ status }: { status: LocalModelStatus | null }) {
  const [question, setQuestion] = useState("");
  const [turns, setTurns] = useState<Turn[]>([]);
  const nextTurnId = useRef(0);
  const conversation = useRef<HTMLDivElement>(null);
  const wasScrolledToBottom = useRef(true);

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
        <h1>Hey Dot</h1>
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
