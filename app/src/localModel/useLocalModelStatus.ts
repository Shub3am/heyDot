// The one subscription to the local model's status, shared by every surface that needs it.
// Must not render anything; ModelStatus.tsx shows the status.

import { useEffect, useState } from "react";
import { watchLocalModel, type LocalModelStatus } from "../chat/ipc";

/** The latest status, or null until the first one arrives. */
export default function useLocalModelStatus(): LocalModelStatus | null {
  const [status, setStatus] = useState<LocalModelStatus | null>(null);

  useEffect(() => {
    void watchLocalModel(setStatus);
  }, []);

  return status;
}
