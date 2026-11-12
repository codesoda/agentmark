/**
 * LinkedIn post identifier lookup. Runs in the page's MAIN world.
 *
 * Posts in the timeline carry no permalink and no ID in the DOM. The ID only
 * exists in the data LinkedIn's own React components hold, which an isolated
 * content script cannot see. This module reads it on request and publishes it
 * as an attribute on the post element, which the isolated script can read.
 *
 * It only reads; it never clicks, navigates or changes LinkedIn's state.
 */

/** Must match `RESOLVE_EVENT` in `linkedin-posts.ts` (the isolated script is a separate bundle). */
export const RESOLVE_EVENT = "agentmark:resolve-post-urn";
/** Must match `URN_ATTR` in `linkedin-posts.ts`. */
export const URN_ATTR = "data-agentmark-post-urn";

const URN_PATTERN = /urn:li:(?:activity|share|ugcPost):\d+/;
/** How far up the component tree to look before giving up. */
const MAX_DEPTH = 40;

interface FiberLike {
  memoizedProps?: unknown;
  return?: FiberLike | null;
}

/**
 * Find the post URN in the React data attached to `element`, walking up the
 * component tree from the element itself so the nearest match wins.
 */
export function findPostUrn(element: Element): string | null {
  const fiberKey = Object.keys(element).find((key) => key.startsWith("__reactFiber"));
  if (!fiberKey) return null;

  let fiber = (element as unknown as Record<string, FiberLike | null | undefined>)[fiberKey];
  for (let depth = 0; fiber && depth < MAX_DEPTH; depth++, fiber = fiber.return) {
    try {
      // Children and callbacks are skipped: they are large, cyclic, or belong to other posts.
      const text = JSON.stringify(fiber.memoizedProps, (key, value) =>
        key === "children" || typeof value === "function" ? undefined : value,
      );
      const match = text?.match(URN_PATTERN);
      if (match) return match[0];
    } catch {
      // Cyclic or otherwise unserializable props at this level — keep climbing.
    }
  }
  return null;
}

/** Answer resolve requests dispatched on post elements by the isolated script. */
export function installUrnResolver(doc: Document): void {
  doc.addEventListener(
    RESOLVE_EVENT,
    (event) => {
      const target = event.target;
      if (!target || typeof (target as Element).setAttribute !== "function") return;
      const element = target as Element;
      const urn = findPostUrn(element);
      if (urn) {
        element.setAttribute(URN_ATTR, urn);
      } else {
        element.removeAttribute(URN_ATTR);
      }
    },
    true,
  );
}
