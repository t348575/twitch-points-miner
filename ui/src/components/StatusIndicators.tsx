import { useLayoutEffect } from "react";
import { Box, Text, Tooltip } from "@mantine/core";
import { IconPick } from "@tabler/icons-react";

import { formatDelta } from "../lib/format";

/**
 * One clock for both indicators, so the halo leaves the dot on the same beat
 * the pickaxe lands.
 *
 * Matching the durations is not enough: a CSS animation starts when its element
 * does, so a channel that goes live - or starts being watched - after the page
 * loaded would beat against the indicators already on screen. Anchoring the
 * delay to a shared epoch puts every indicator on the same phase whenever it
 * mounts. Keep in step with `--tpm-period` in index.css.
 */
/**
 * One clock for both indicators, so the halo leaves the dot on the same beat
 * the pickaxe lands.
 *
 * Matching the durations is not enough, because a CSS animation starts when its
 * element does: a channel that goes live - or starts being watched - after the
 * page loaded would beat against the indicators already on screen. Computing a
 * negative `animation-delay` at render does not fix it either, since React
 * renders well before the browser actually starts the animation, and that gap
 * is the error.
 *
 * So align the animations themselves. Pinning every one to the same
 * `startTime` puts them all at the same point of the cycle, whenever they
 * mounted. Runs on every render, which also re-syncs after the animations are
 * recreated - switching the motion preference back on, for instance.
 */
const SYNCED = new Set(["tpm-strike", "tpm-ping"]);

function useSyncedBeat(): void {
  useLayoutEffect(() => {
    for (const animation of document.getAnimations()) {
      if ("animationName" in animation && SYNCED.has((animation as CSSAnimation).animationName)) {
        // Read-only while an animation is still being set up.
        try {
          animation.startTime = 0;
        } catch {
          /* not ready yet; the next render catches it */
        }
      }
    }
  });
}

/** Pulsing dot shown next to a live channel. */
export function LiveDot() {
  useSyncedBeat();

  return (
    <Tooltip label="Live">
      <Box
        component="span"
        className="tpm-live"
        aria-label="Live"
        style={{
          width: 8,
          height: 8,
          borderRadius: "50%",
          background: "var(--tpm-live)",
          boxShadow: "0 0 0 3px color-mix(in srgb, var(--tpm-live) 22%, transparent)",
          display: "inline-block",
          flexShrink: 0,
        }}
      />
    </Tooltip>
  );
}

/** Shown for the channels the miner is actively watching. */
export function MiningIcon() {
  useSyncedBeat();

  return (
    <Tooltip label="Mining now">
      <IconPick
        size={16}
        className="tpm-mining"
        color="var(--mantine-primary-color-filled)"
        aria-label="Mining now"
      />
    </Tooltip>
  );
}

/** Signed points change, coloured green or red. Zero renders as a dash. */
export function PointsDelta({ value, size = "sm" }: { value: number; size?: string }) {
  if (value === 0) {
    return (
      <Text size={size} c="dimmed">
        &mdash;
      </Text>
    );
  }
  return (
    <Text className="tpm-num" size={size} fw={600} c={value > 0 ? "teal" : "red"}>
      {formatDelta(value)}
    </Text>
  );
}
