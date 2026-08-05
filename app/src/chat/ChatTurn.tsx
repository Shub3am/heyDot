// One question and its answer as the conversation shows them.
// Must not change a turn or talk to the backend beyond the settings button; ChatPanel owns the turns.

import { memo } from "react";
import Markdown from "react-markdown";
import rehypeHighlight from "rehype-highlight";
import remarkGfm from "remark-gfm";
import CopyAnswerButton from "./CopyAnswerButton";
import { openScreenRecordingSettings, type ScreenShare } from "./ipc";

export type Turn = {
  id: number;
  question: string;
  text: string;
  badge: string | null;
  screen: ScreenShare | null;
  error: string | null;
};

const REMARK_PLUGINS = [remarkGfm];
const REHYPE_PLUGINS = [rehypeHighlight];

function describeMissingScreenshot(screen: ScreenShare) {
  switch (screen.kind) {
    case "attached":
      return null;
    case "permissionNeeded":
      return (
        <p role="status" className="notice">
          Answered without a screenshot: Hey Dot needs Screen Recording permission. Turn on Hey Dot in System
          Settings, then quit and reopen Hey Dot.{" "}
          <button onClick={() => void openScreenRecordingSettings()}>Open Screen Recording settings</button>
        </p>
      );
    case "failed":
      return (
        <p role="status" className="notice">
          Answered without a screenshot: {screen.reason}
        </p>
      );
  }
}

// Memoised because every streamed delta re-renders the panel; only the turn whose object changed re-parses its markdown.
export default memo(function ChatTurn({ turn }: { turn: Turn }) {
  const hasScreenshot = turn.screen?.kind === "attached";
  return (
    <article className="turn">
      <p className="question">{turn.question}</p>
      <div className="answer">
        {(turn.badge || hasScreenshot) && (
          <p className="meta">
            {turn.badge && <span>{turn.badge}</span>}
            {hasScreenshot && <span>With a screenshot of this screen</span>}
          </p>
        )}
        {turn.screen && describeMissingScreenshot(turn.screen)}
        <div className="answer-text">
          <Markdown remarkPlugins={REMARK_PLUGINS} rehypePlugins={REHYPE_PLUGINS}>
            {turn.text}
          </Markdown>
        </div>
        {turn.text && <CopyAnswerButton answer={turn.text} />}
        {turn.error && (
          <p role="alert" className="error">
            {turn.error}
          </p>
        )}
      </div>
    </article>
  );
});
