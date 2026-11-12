import { describe, expect, it } from "vitest";
import { isLiveConnection, lastSeenText } from "./connection";
import { CONNECTION, STALE_CONNECTION } from "./mock/fixtures";

const STARTED = Date.parse("2026-11-12T10:00:00.500Z");
const at = (iso: string) => ({ ...CONNECTION, connected_at: iso });

describe("isLiveConnection (review finding #12)", () => {
  it("is live only for a record written after the app started", () => {
    expect(isLiveConnection(at("2026-11-12T10:00:05Z"), STARTED)).toBe(true);
    // Whole seconds: the same second the app started still counts.
    expect(isLiveConnection(at("2026-11-12T10:00:00Z"), STARTED)).toBe(true);
    expect(isLiveConnection(at("2026-11-12T09:59:30Z"), STARTED)).toBe(false);
    expect(isLiveConnection(STALE_CONNECTION, STARTED)).toBe(false);
  });

  it("is not live without a record or with an unreadable time", () => {
    expect(isLiveConnection(null, STARTED)).toBe(false);
    expect(isLiveConnection(at("yesterday"), STARTED)).toBe(false);
  });

  it("describes an old record as last seen", () => {
    expect(lastSeenText(STALE_CONNECTION)).toMatch(/^Last connected .+ · version 0\.2\.6$/);
    expect(lastSeenText({ ...STALE_CONNECTION, connected_at: "?", extension_version: null })).toBe("Connected before");
  });
});
