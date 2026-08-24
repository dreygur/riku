"use client";

import { useState } from "react";
import { toast } from "sonner";

/**
 * Runs an async action, tracking which one (by `key`) is in flight so
 * callers can disable just that control and show a "…ing" state instead of
 * only reacting after the fact: a request that takes a couple of seconds
 * (a marketplace clone, a scale) otherwise looks like nothing happened, or
 * like it silently failed, until the toast finally appears.
 */
export function usePendingRun() {
  const [pending, setPending] = useState<string | null>(null);

  async function run(key: string, label: string, fn: () => Promise<void>) {
    setPending(key);
    try {
      await fn();
      toast.success(label);
    } catch (e) {
      toast.error(`${label} failed: ${(e as Error).message}`);
    } finally {
      setPending(null);
    }
  }

  return { pending, isPending: (key: string) => pending === key, run };
}
