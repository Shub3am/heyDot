// The first chat surface: the local model's state, one typed question and its streamed answer.
// Must not call invoke directly; every backend call goes through ./ipc.

import { useEffect, useState } from "react";
import {
  askText,
  downloadLocalModel,
  openScreenRecordingSettings,
  watchLocalModel,
  type LocalModelStatus,
  type ScreenShare,
} from "./ipc";

const SERVER_LOG_PATH = "~/Library/Logs/Hey Dot/llama-server.log";

type Answer = {
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
  const [answer, setAnswer] = useState<Answer | null>(null);
  const [asking, setAsking] = useState(false);

  useEffect(() => {
    void watchLocalModel(setStatus);
  }, []);

  async function ask() {
    setAsking(true);
    setQuestion("");
    setAnswer({ question, text: "", badge: null, screen: null, error: null });
    try {
      await askText(question, (event) =>
        setAnswer((current) => {
          if (current === null) return current;
          if (event.event === "started") {
            const badge = event.data.leavesDevice ? `Sent to ${event.data.host}` : "On this Mac";
            return { ...current, badge, screen: event.data.screen };
          }
          return { ...current, text: current.text + event.data.text };
        }),
      );
    } catch (error) {
      setAnswer((current) => current && { ...current, error: String(error) });
    } finally {
      setAsking(false);
    }
  }

  const canAsk = status?.phase.kind === "ready" && !asking && question.trim() !== "";

  return (
    <section>
      {describeModel(status)}
      <label>
        Question
        <textarea value={question} onChange={(event) => setQuestion(event.target.value)} />
      </label>
      <button disabled={!canAsk} onClick={() => void ask()}>
        Ask
      </button>
      {answer && (
        <article>
          <p>{answer.question}</p>
          {answer.badge && <p>{answer.badge}</p>}
          {answer.screen && describeScreenShare(answer.screen)}
          <p style={{ whiteSpace: "pre-wrap" }}>{answer.text}</p>
          {answer.error && <p role="alert">{answer.error}</p>}
        </article>
      )}
    </section>
  );
}
