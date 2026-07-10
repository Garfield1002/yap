import { describe, expect, it } from "vitest";
import { BASELINE_GRID_PX, calculateBaselineCalibration, gridAlignedHeight, snapToBaselineGrid } from "./baselineGrid";

describe("baseline grid helpers", () => {
  it("snaps measurements using the requested direction", () => {
    expect(snapToBaselineGrid(35)).toBe(24);
    expect(snapToBaselineGrid(35, "up")).toBe(48);
    expect(snapToBaselineGrid(35, "down")).toBe(24);
  });

  it("rounds expanding raw and rendered blocks upward", () => {
    expect(gridAlignedHeight(0)).toBe(BASELINE_GRID_PX);
    expect(gridAlignedHeight(49)).toBe(72);
    expect(gridAlignedHeight(25, 96)).toBe(96);
  });

  it("puts the first body baseline at 48px and headings on its phase", () => {
    const measured = { body: 18, h1: 35, h2: 31, h3: 29, h4: 19, h5: 18, h6: 17 };
    const result = calculateBaselineCalibration(measured);
    expect(result.contentPaddingTop + measured.body).toBe(48);
    expect(result.blockInset).toBe(measured.body);
    expect(measured.h1 + result.headingShifts[0] + result.contentPaddingTop).toBe(48);
    result.headingShifts.forEach((shift, index) => {
      expect(measured[`h${index + 1}` as keyof typeof measured] + shift + result.contentPaddingTop).toBe(48);
    });
  });
});
