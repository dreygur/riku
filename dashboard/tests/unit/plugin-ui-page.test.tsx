import { act, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import PluginUiPage from "@/app/plugins/[name]/page";
import { api } from "@/lib/api";
import type { UiPanel } from "@/lib/types";

vi.mock("@/lib/api", () => ({ api: { pluginUi: vi.fn() } }));

const pluginUi = vi.mocked(api.pluginUi);

// The page reads its route param through React's `use`. A promise already
// tagged fulfilled (React's thenable protocol) unwraps synchronously, so the
// page renders without a Suspense boundary or an async flush in the test.
function params(name: string): Promise<{ name: string }> {
  return Object.assign(Promise.resolve({ name }), { status: "fulfilled", value: { name } });
}

function renderPage(name = "demo") {
  return render(<PluginUiPage params={params(name)} />);
}

// The page's failure branch (api.pluginUi rejects -> "has no UI panel") is not
// unit-tested here: the page handles the rejection with its own `.catch`, but
// React 19 re-surfaces that handled rejection to the runner's unhandled-
// rejection guard a macrotask later, so the only ways to make it pass are to
// silence unhandled errors globally or to hack test timing. That branch is
// left to the Playwright e2e suite, which drives a real browser.
describe("PluginUiPage", () => {
  beforeEach(() => pluginUi.mockReset());

  it("renders every section title with its field labels and values", async () => {
    pluginUi.mockResolvedValue({
      sections: [{ title: "Status", fields: [{ label: "state", value: "running" }] }],
    });

    renderPage("redis");

    expect(pluginUi).toHaveBeenCalledWith("redis");
    expect(await screen.findByText("Status")).toBeInTheDocument();
    expect(screen.getByText("state")).toBeInTheDocument();
    expect(screen.getByText("running")).toBeInTheDocument();
  });

  it("shows the empty-panel notice when the plugin returns no sections", async () => {
    pluginUi.mockResolvedValue({ sections: [] });

    renderPage();

    expect(await screen.findByText("this plugin returned an empty panel")).toBeInTheDocument();
  });

  it("shows a loading state until the panel resolves", async () => {
    let resolvePanel!: (panel: UiPanel) => void;
    pluginUi.mockReturnValue(new Promise<UiPanel>((resolve) => (resolvePanel = resolve)));

    renderPage();

    expect(await screen.findByText("loading…")).toBeInTheDocument();

    await act(async () => resolvePanel({ sections: [] }));

    expect(await screen.findByText("this plugin returned an empty panel")).toBeInTheDocument();
  });
});
