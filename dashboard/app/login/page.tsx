"use client";

import { Suspense, useState, type FormEvent } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { Button } from "@/components/ui/button";

export const dynamic = "force-dynamic";

export default function LoginPage() {
  return (
    <Suspense>
      <LoginForm />
    </Suspense>
  );
}

function LoginForm() {
  const router = useRouter();
  const params = useSearchParams();
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setPending(true);
    setError("");
    try {
      const res = await fetch("/api/login", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ password }),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        setError(body.error ?? "login failed");
        return;
      }
      router.replace(params.get("from") || "/");
      router.refresh();
    } finally {
      setPending(false);
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center px-5">
      <form onSubmit={submit} className="w-full max-w-xs border border-border bg-card p-6">
        <h1 className="mb-1 font-mono text-lg font-bold">
          riku<span className="text-primary">▌</span>
        </h1>
        <p className="mb-5 font-sans text-xs text-muted-foreground">
          enter the dashboard password to continue
        </p>
        <input
          type="password"
          autoFocus
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          placeholder="password"
          className="mb-3 w-full border border-input bg-background px-3 py-2 font-mono text-sm text-foreground outline-none focus:border-ring"
        />
        {error && <p className="mb-3 font-sans text-xs text-destructive">{error}</p>}
        <Button type="submit" variant="accent" className="w-full" disabled={pending || !password}>
          {pending ? "checking…" : "unlock"}
        </Button>
      </form>
    </div>
  );
}
