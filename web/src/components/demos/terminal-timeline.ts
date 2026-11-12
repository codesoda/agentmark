/*
 * Turns a session into a flat list of steps, each a cursor plus the delay
 * before it. The demo keeps one number (the step index) in state, so
 * pausing, resuming and replaying are just "schedule the next step or not".
 */

import type { Block } from "./terminal-script";

/** Blocks before `block` are complete; `block` itself shows `part` parts. */
export type Cursor = { block: number; part: number };
export type Step = { delay: number; cursor: Cursor };

/** How many parts a block reveals in. */
export function partsOf(block: Block) {
  switch (block.kind) {
    case "type":
      return block.text.length + (block.paste ? 1 : 0);
    case "out":
      return Math.ceil(block.lines.length / block.chunk);
    case "tool":
      return 1;
    case "say":
      return block.sentences.length;
  }
}

/** Uneven but repeatable keystrokes, a little slower after spaces. */
function keystroke(text: string, index: number) {
  const base = 34 + ((index * 37) % 7) * 7;
  return text[index - 1] === " " ? base + 45 : base;
}

function partDelay(block: Block, part: number) {
  switch (block.kind) {
    case "type":
      if (part === 1) return 550;
      return part > block.text.length ? 300 : keystroke(block.text, part - 1);
    case "out":
      return part === 1 ? 240 : 70;
    case "tool":
      return 850;
    case "say":
      return block.quiet ? 380 : part === 1 ? 520 : 700;
  }
}

/** Pause after a block is complete, before the next one starts. */
function settleDelay(block: Block) {
  switch (block.kind) {
    case "type":
      return 420;
    case "out":
      return 650;
    case "tool":
      return 360;
    case "say":
      return 260;
  }
}

export function buildTimeline(blocks: readonly Block[]): Step[] {
  const steps: Step[] = [{ delay: 0, cursor: { block: 0, part: 0 } }];
  blocks.forEach((block, i) => {
    for (let part = 1; part <= partsOf(block); part++) {
      steps.push({ delay: partDelay(block, part), cursor: { block: i, part } });
    }
    steps.push({ delay: settleDelay(block), cursor: { block: i + 1, part: 0 } });
  });
  return steps;
}
