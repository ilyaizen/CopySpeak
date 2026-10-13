import { describe, it, expect } from "vite-plus/test";
import { choicesWithCurrent } from "./select-choices";

describe("choicesWithCurrent", () => {
  const choices = ["eleven_v4", "eleven_v3"];

  it("returns the choices unchanged when the current value is listed", () => {
    expect(choicesWithCurrent(choices, "eleven_v3")).toEqual(["eleven_v4", "eleven_v3"]);
  });

  it("appends a saved value that is not in the list so it stays selectable", () => {
    expect(choicesWithCurrent(choices, "eleven_turbo_v2_5")).toEqual([
      "eleven_v4",
      "eleven_v3",
      "eleven_turbo_v2_5"
    ]);
  });

  it("returns the choices unchanged for an empty, null or undefined value", () => {
    expect(choicesWithCurrent(choices, "")).toEqual(["eleven_v4", "eleven_v3"]);
    expect(choicesWithCurrent(choices, null)).toEqual(["eleven_v4", "eleven_v3"]);
    expect(choicesWithCurrent(choices, undefined)).toEqual(["eleven_v4", "eleven_v3"]);
  });

  it("does not mutate the input list", () => {
    const input = ["a"];
    choicesWithCurrent(input, "b");
    expect(input).toEqual(["a"]);
  });
});
