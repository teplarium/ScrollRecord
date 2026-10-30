import { describe, expect, it } from "vitest";

import { parentFolder } from "./dialogs";

describe("parentFolder", () => {
  it("drops the file name with either separator", () => {
    expect(parentFolder("E:\\Проекты\\site\\index.html")).toBe("E:\\Проекты\\site");
    expect(parentFolder("/home/me/site/index.html")).toBe("/home/me/site");
    expect(parentFolder("C:/mixed\\site/index.html")).toBe("C:/mixed\\site");
  });

  it("keeps the root folder whole", () => {
    expect(parentFolder("C:\\index.html")).toBe("C:\\");
    expect(parentFolder("/index.html")).toBe("/");
  });
});
