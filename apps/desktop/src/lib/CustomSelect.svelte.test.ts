import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import CustomSelect from "./CustomSelect.svelte";

const options = [
  { value: "local", label: "Local" },
  { value: "cloud", label: "Cloud" },
];

describe("CustomSelect", () => {
  it("renders the selected label and emits the chosen value", async () => {
    const onChange = vi.fn();
    render(CustomSelect, {
      props: { value: "local", options },
      events: { change: onChange },
    });

    const trigger = screen.getByRole("button", { name: "Local" });
    expect(trigger).toHaveAttribute("type", "button");

    await fireEvent.click(trigger);
    expect(screen.getByRole("button", { name: "Cloud" })).toBeVisible();

    await fireEvent.click(screen.getByRole("button", { name: "Cloud" }));
    expect(onChange).toHaveBeenCalledOnce();
    expect(onChange.mock.calls[0][0]).toMatchObject({ detail: "cloud" });
    expect(screen.queryByRole("button", { name: "Local" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cloud" })).toBeVisible();
  });

  it("does not open while disabled", async () => {
    render(CustomSelect, { value: "local", options, disabled: true });

    const trigger = screen.getByRole("button", { name: "Local" });
    expect(trigger).toBeDisabled();
    await fireEvent.click(trigger);

    expect(screen.queryByRole("button", { name: "Cloud" })).not.toBeInTheDocument();
  });

  it("closes without changing the value when the backdrop is clicked", async () => {
    const { container } = render(CustomSelect, { value: "local", options });

    await fireEvent.click(screen.getByRole("button", { name: "Local" }));
    const backdrop = container.querySelector(".select-backdrop");
    expect(backdrop).not.toBeNull();
    await fireEvent.click(backdrop as HTMLElement);

    expect(screen.queryByRole("button", { name: "Cloud" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Local" })).toBeVisible();
  });
});
