import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ExtensionEntry } from "../../types";
import { ExtensionsSettings } from "../ExtensionsSettings";

const mockUseExtensionsSettings = vi.fn();

vi.mock("@/features/extensions/hooks/useExtensionsSettings", () => ({
  useExtensionsSettings: () => mockUseExtensionsSettings(),
}));

const extensions: ExtensionEntry[] = [
  {
    type: "stdio",
    name: "github",
    description: "Issue tracker",
    cmd: "npx",
    args: [],
    config_key: "github",
    enabled: true,
  },
  {
    type: "builtin",
    name: "developer",
    display_name: "Developer",
    description: "Code tools",
    config_key: "developer",
    enabled: true,
  },
  {
    type: "platform",
    name: "summarize",
    display_name: "Summarize",
    description: "Summarize files",
    config_key: "summarize",
    enabled: false,
  },
];

describe("ExtensionsSettings", () => {
  let handleToggleEnabled: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    handleToggleEnabled = vi.fn();
    mockUseExtensionsSettings.mockReturnValue({
      extensions,
      isLoading: false,
      modalMode: null,
      editingExtension: null,
      handleAdd: vi.fn(),
      handleConfigure: vi.fn(),
      handleSubmit: vi.fn(),
      handleDelete: vi.fn(),
      handleToggleEnabled,
      handleModalClose: vi.fn(),
    });
  });

  it("reveals matching Goose capabilities while searching", async () => {
    const user = userEvent.setup();
    render(<ExtensionsSettings />);

    expect(screen.queryByText("Developer")).not.toBeInTheDocument();

    await user.type(screen.getByRole("searchbox"), "developer");

    expect(screen.getByText("Developer")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", {
        name: /show .*built-in goose capabilities/i,
      }),
    ).not.toBeInTheDocument();
  });

  it("shows an enable toggle for default-off Goose capabilities", async () => {
    const user = userEvent.setup();
    render(<ExtensionsSettings />);

    await user.type(screen.getByRole("searchbox"), "summarize");
    await user.click(screen.getByRole("switch", { name: /enable summarize/i }));

    expect(handleToggleEnabled).toHaveBeenCalledWith(
      expect.objectContaining({ name: "summarize" }),
      true,
    );
    expect(
      screen.queryByRole("switch", { name: /enable developer/i }),
    ).not.toBeInTheDocument();
  });
});
