import type { Metadata } from "next";
import { Toaster } from "@/components/ui/sonner";
import { TopNav } from "@/components/riku/top-nav";
import { CommandMenu } from "@/components/riku/command-menu";
import "./globals.css";

export const metadata: Metadata = {
  title: "riku // dashboard",
  description: "Live supervisor, deploy, and metrics console for riku.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  // `dark` is forced — the dashboard is dark-only by design.
  return (
    <html lang="en" className="dark" suppressHydrationWarning>
      <body>
        <TopNav />
        <main className="mx-auto max-w-5xl px-5 py-7">{children}</main>
        <CommandMenu />
        <Toaster position="bottom-right" />
      </body>
    </html>
  );
}
