import { useComputedColorScheme, useMantineTheme } from "@mantine/core";
import { useMemo } from "react";

import { areaGradient } from "./colors";

export interface ChartTheme {
  dark: boolean;
  axis: string;
  label: string;
  split: string;
  tooltipBg: string;
  tooltipBorder: string;
  tooltipText: string;
  accent: string;
  /** Vertical fade for an area fill, matched to `accent`. */
  accentArea: ReturnType<typeof areaGradient>;
}

/** Colours for ECharts options, matched to the current Mantine colour scheme. */
export function useChartTheme(): ChartTheme {
  const theme = useMantineTheme();
  const scheme = useComputedColorScheme("light");

  return useMemo(() => {
    const dark = scheme === "dark";
    const accent = theme.colors.brass[dark ? 4 : 6];

    return {
      dark,
      axis: dark ? theme.colors.graphite[4] : theme.colors.gray[3],
      label: dark ? theme.colors.graphite[2] : theme.colors.gray[6],
      split: dark ? theme.colors.graphite[5] : theme.colors.gray[2],
      tooltipBg: dark ? theme.colors.graphite[6] : theme.white,
      tooltipBorder: dark ? theme.colors.graphite[4] : theme.colors.gray[3],
      tooltipText: dark ? theme.colors.gray[2] : theme.colors.graphite[7],
      accent,
      accentArea: areaGradient(accent),
    };
  }, [scheme, theme]);
}
