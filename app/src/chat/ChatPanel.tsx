// The first chat surface: a conversation with the local model, typed one question at a time.
// Must not call invoke directly; every backend call goes through ./ipc.

import { useEffect, useRef, useState } from "react";
import {
  askText,
  downloadLocalModel,
  newChat,
  openScreenRecordingSettings,
  watchLocalModel,
  type LocalModelStatus,
  type ScreenShare,
} from "./ipc";

const SERVER_LOG_PATH = "~/Library/Logs/Hey Dot/llama-server.log";

type Turn = {
  id: number;
  question: string;
  text: string;
  badge: string | null;
  screen: ScreenShare | null;
  error: string | null;
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

function describeScreenShare(screen: ScreenShare) {
  switch (screen.kind) {
    case "attached":
      return <p>With a screenshot of this screen</p>;
    case "permissionNeeded":
      return (
        <p role="status">
          Answered without a screenshot: Hey Dot needs Screen Recording permission. Turn on Hey Dot in System
          Settings, then quit and reopen Hey Dot.{" "}
          <button onClick={() => void openScreenRecordingSettings()}>Open Screen Recording settings</button>
        </p>
      );
    case "failed":
      return <p role="status">Answered without a screenshot: {screen.reason}</p>;
  }
}

export default function ChatPanel() {
  const [status, setStatus] = useState<LocalModelStatus | null>(null);
  const [question, setQuestion] = useState("");
  const [turns, setTurns] = useState<Turn[]>([]);
  const nextTurnId = useRef(0);

  useEffect(() => {
    void watchLocalModel(setStatus);
  }, []);

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

  return (
    <section>
      {describeModel(status)}
      {turns.map((turn) => (
        <article key={turn.id}>
          <p>{turn.question}</p>
          {turn.badge && <p>{turn.badge}</p>}
          {turn.screen && describeScreenShare(turn.screen)}
          <p style={{ whiteSpace: "pre-wrap" }}>{turn.text}</p>
          {turn.error && <p role="alert">{turn.error}</p>}
        </article>
      ))}
      <label>
        Question
        <textarea value={question} onChange={(event) => setQuestion(event.target.value)} />
      </label>
      <button disabled={!canAsk} onClick={() => void ask()}>
        Ask
      </button>
      <button onClick={startNewChat}>New chat</button>
    </section>
  );
}
