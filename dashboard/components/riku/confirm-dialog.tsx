"use client";

import { useCallback, useEffect, useState } from "react";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";

type Request = { message: string; resolve: (ok: boolean) => void };

// Module-level trigger, wired up once ConfirmHost mounts (in layout.tsx) --
// mirrors the "riku-open-command" window-event pattern CommandMenu uses to
// let any component reach a singleton without prop-drilling. Falls back to
// window.confirm only in the (never-expected) case nothing has mounted yet.
let trigger: (message: string) => Promise<boolean> = (message) =>
  Promise.resolve(typeof window !== "undefined" ? window.confirm(message) : false);

/** Replaces `window.confirm` with the app's own styled dialog. */
export function confirmDialog(message: string): Promise<boolean> {
  return trigger(message);
}

/** Mount once, near the root layout. Renders whatever confirmDialog() last asked for. */
export function ConfirmHost() {
  const [req, setReq] = useState<Request | null>(null);

  useEffect(() => {
    trigger = (message: string) => new Promise((resolve) => setReq({ message, resolve }));
  }, []);

  const close = useCallback(
    (ok: boolean) => {
      req?.resolve(ok);
      setReq(null);
    },
    [req],
  );

  return (
    <AlertDialog open={req !== null} onOpenChange={(open) => !open && close(false)}>
      <AlertDialogContent className="font-mono">
        <AlertDialogHeader>
          <AlertDialogTitle>Are you sure?</AlertDialogTitle>
          <AlertDialogDescription>{req?.message}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel onClick={() => close(false)}>cancel</AlertDialogCancel>
          <AlertDialogAction onClick={() => close(true)}>confirm</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
