/**
 * Minimal ECharts wrapper.
 *
 * Imports come from `echarts/core` with an explicit `use()` list rather than the
 * top level `echarts` barrel, which would pull in every chart type and roughly
 * double the bundle. That matters here because the dashboard is mobile first.
 */

import { useEffect, useRef } from "react";
import * as echarts from "echarts/core";
import { LineChart } from "echarts/charts";
import {
  DataZoomComponent,
  GridComponent,
  LegendComponent,
  MarkLineComponent,
  TooltipComponent,
} from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";
import type { EChartsOption } from "echarts";

echarts.use([
  LineChart,
  GridComponent,
  TooltipComponent,
  LegendComponent,
  DataZoomComponent,
  MarkLineComponent,
  CanvasRenderer,
]);

interface EChartProps {
  option: EChartsOption;
  height: number | string;
  /** Drop the previous series instead of merging into it. */
  replaceMerge?: string[];
}

export function EChart({ option, height, replaceMerge }: EChartProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<echarts.ECharts | null>(null);

  useEffect(() => {
    const element = containerRef.current;
    if (!element) return;

    // No theme argument: the built-in "dark" theme is not registered when
    // importing from echarts/core, so callers colour the option themselves.
    const chart = echarts.init(element, undefined, { renderer: "canvas" });
    chartRef.current = chart;

    // ECharts cannot size itself; watch the container instead.
    const observer = new ResizeObserver(() => chart.resize());
    observer.observe(element);

    return () => {
      observer.disconnect();
      chart.dispose();
      chartRef.current = null;
    };
  }, []);

  useEffect(() => {
    chartRef.current?.setOption(option, { replaceMerge });
  }, [option, replaceMerge]);

  return <div ref={containerRef} style={{ width: "100%", height, background: "transparent" }} />;
}
