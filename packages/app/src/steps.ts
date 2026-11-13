/** The three launch steps, in the order they run (spec §5). */

import type { StepId, StepReport } from "./types";

export type StepState = "pending" | "running" | StepReport;

export const STEP_ORDER: StepId[] = ["cli", "native_host", "skill"];

export const STEP_TITLES: Record<StepId, string> = {
  cli: "Install the agentmark command",
  native_host: "Register with Chrome",
  skill: "Add the agent skill",
};

