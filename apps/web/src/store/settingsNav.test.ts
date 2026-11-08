import { describe, expect, it } from "vitest";
import { reduce } from "./reduce";
import type { Route } from "./route";
import { backRoute, nextSettingsFrom } from "./settingsNav";
import { initialState } from "./state";

const card: Route = { name: "card", project: "DEMO", chain: "demo", key: "DEMO-3" };
const settings: Route = { name: "settings", project: "DEMO" };

describe("Back to work", () => {
  it("remembers the page the settings were opened from", () => {
    expect(nextSettingsFrom(card, settings, undefined)).toEqual(card);
  });

  it("keeps it while moving among settings pages", () => {
    expect(nextSettingsFrom(settings, { name: "settings", section: "about" }, card)).toEqual(card);
  });

  it("leaves it alone on other moves, and goes home without one", () => {
    expect(nextSettingsFrom(card, { name: "inbox" }, undefined)).toBeUndefined();
    expect(backRoute(undefined)).toEqual({ name: "chains" });
    expect(backRoute(settings)).toEqual({ name: "chains" });
    expect(backRoute(card)).toEqual(card);
  });

  it("the store keeps it across settings pages and back-button moves", () => {
    const [s1] = reduce(initialState(card), { type: "navigate", route: settings });
    const [s2] = reduce(s1, { type: "navigate", route: { name: "settings", section: "checks" } });
    expect(s2.settingsFrom).toEqual(card);
    const [s3] = reduce(s2, { type: "urlChanged", route: { name: "settings", project: "DEMO", section: "review" } });
    expect(s3.settingsFrom).toEqual(card);
    const [s4] = reduce(s3, { type: "navigate", route: backRoute(s3.settingsFrom) });
    expect(s4.route).toEqual(card);
  });
});
