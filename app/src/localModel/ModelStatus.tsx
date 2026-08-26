// The local model's status as one coloured dot and one sentence, with the button that fixes it when there is one.
// Must not subscribe to the status; the caller passes what useLocalModelStatus returns.

import { downloadLocalModel, type LocalModelPhase, type LocalModelStatus } from "../chat/ipc";
import "./ModelStatus.css";

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

export default function ModelStatus({ status }: { status: LocalModelStatus | null }) {
  return (
    <div className="model-status" data-dot={status ? STATUS_DOT[status.phase.kind] : "busy"}>
      {describeModel(status)}
    </div>
  );
}
