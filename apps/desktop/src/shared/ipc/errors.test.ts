import { describe, expect, it } from "vitest";

import { appError, isAppError, toAppError } from "./errors";

describe("toAppError", () => {
  it("passes a real AppError through unchanged", () => {
    const err = appError("SR-0002", "planned", { phase: "3" });
    expect(toAppError(err)).toBe(err);
  });

  it("wraps a JS Error as SR-0001", () => {
    const err = toAppError(new Error("boom"));
    expect(err.code).toBe("SR-0001");
    expect(err.message).toContain("boom");
  });

  it("does not trust objects with unknown codes", () => {
    expect(isAppError({ code: "SR-9999", message: "x", params: {} })).toBe(false);
    expect(toAppError("plain string").code).toBe("SR-0001");
  });
});
