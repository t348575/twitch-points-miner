import { useMemo } from "react";
import type { EChartsOption } from "echarts";

import { EChart } from "../../components/EChart";
import { useChartTheme } from "../../lib/useChartTheme";
import { formatDelta, formatTime } from "../../lib/format";
import { endOfToday, startOfToday } from "../../api/time";

interface TodaySparklineProps {
  /** Cumulative gain as [timestamp, value] pairs. */
  data: [number, number][];
}

export function TodaySparkline({ data }: TodaySparklineProps) {
  const chart = useChartTheme();

  const option = useMemo<EChartsOption>(() => {
    const now = Date.now();

    return {
      animation: false,
      grid: { left: 8, right: 8, top: 12, bottom: 8, containLabel: true },
      xAxis: {
        type: "time",
        min: startOfToday().getTime(),
        max: endOfToday().getTime(),
        axisLine: { lineStyle: { color: chart.axis } },
        splitNumber: 4,
        axisLabel: {
          color: chart.label,
          fontSize: 10,
          hideOverlap: true,
          formatter: (value: number) => formatTime(new Date(value)),
        },
        splitLine: { show: false },
      },
      yAxis: {
        type: "value",
        axisLabel: { color: chart.label, fontSize: 10 },
        splitLine: { lineStyle: { color: chart.split } },
      },
      tooltip: {
        trigger: "axis",
        backgroundColor: chart.tooltipBg,
        borderColor: chart.tooltipBorder,
        textStyle: { color: chart.tooltipText, fontSize: 12 },
        formatter: (params: unknown) => {
          const list = params as { value: [number, number] }[];
          const point = list[0];
          if (!point) return "";
          return `${formatTime(new Date(point.value[0]))}<br/><b>${formatDelta(point.value[1])}</b> today`;
        },
      },
      series: [
        {
          type: "line",
          data,
          showSymbol: false,
          step: "end",
          lineStyle: { width: 2, color: chart.accent },
          areaStyle: { color: chart.accentArea },
          markLine: {
            silent: true,
            symbol: "none",
            label: { show: false },
            lineStyle: { color: chart.axis, type: "dashed", width: 1 },
            data: [{ xAxis: now }],
          },
        },
      ],
    };
  }, [data, chart]);

  return <EChart option={option} height="100%" />;
}
