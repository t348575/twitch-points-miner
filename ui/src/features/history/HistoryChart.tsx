import { useMemo } from "react";
import type { EChartsOption } from "echarts";

import { EChart } from "../../components/EChart";
import { useChartTheme } from "../../lib/useChartTheme";
import { channelColor } from "../../lib/colors";
import { formatDateTime, formatDelta, formatPoints } from "../../lib/format";
import { parseNaiveLocal } from "../../api/time";
import type { PointsInfo, TimelineResult } from "../../api/types";

export type HistoryMode = "total" | "perChannel";

interface HistoryChartProps {
  rows: TimelineResult[];
  mode: HistoryMode;
  /** Channel id to display name. */
  names: Map<number, string>;
  /** Rows before this instant are context for the LAG delta only. */
  since: Date;
  height: number;
  showSlider: boolean;
}

/** Human wording for why a points row was written. */
function describeReason(info: PointsInfo, predictionTitle: string | null): string {
  if (info === "FirstEntry") return "Streamer added";
  if (info === "Watching") return "Watching";
  if (info === "CommunityPointsClaimed") return "Community bonus claim";
  return predictionTitle ? `Prediction - ${predictionTitle}` : "Prediction";
}

export function HistoryChart({ rows, mode, names, since, height, showSlider }: HistoryChartProps) {
  const chart = useChartTheme();

  const option = useMemo<EChartsOption>(() => {
    const visible = rows.filter((r) => parseNaiveLocal(r.point.created_at) >= since);

    let series: EChartsOption["series"];

    if (mode === "perChannel") {
      // Raw balances per channel.
      const byChannel = new Map<number, [number, number][]>();
      for (const row of visible) {
        const at = parseNaiveLocal(row.point.created_at).getTime();
        const list = byChannel.get(row.point.channel_id);
        const entry: [number, number] = [at, row.point.points_value];
        if (list) list.push(entry);
        else byChannel.set(row.point.channel_id, [entry]);
      }

      series = [...byChannel.entries()].map(([channelId, data]) => ({
        type: "line" as const,
        name: names.get(channelId) ?? String(channelId),
        data,
        showSymbol: false,
        connectNulls: false,
        lineStyle: { width: 2, color: channelColor(channelId) },
        itemStyle: { color: channelColor(channelId) },
      }));
    } else {
      // Cumulative gain across all selected channels. Summing raw balances would
      // jump whenever a channel joins the set, so sum the deltas instead.
      const sorted = visible.toSorted(
        (a, b) =>
          parseNaiveLocal(a.point.created_at).getTime() -
          parseNaiveLocal(b.point.created_at).getTime(),
      );

      const data: [number, number][] = [[since.getTime(), 0]];
      let running = 0;
      for (const row of sorted) {
        if (row.difference == null) continue;
        running += row.difference;
        data.push([parseNaiveLocal(row.point.created_at).getTime(), running]);
      }

      series = [
        {
          type: "line",
          name: "Total gained",
          data,
          showSymbol: false,
          step: "end",
          lineStyle: { width: 2, color: chart.accent },
          areaStyle: { color: chart.accentArea },
        },
      ];
    }

    return {
      animation: false,
      grid: { left: 8, right: 12, top: 12, bottom: showSlider ? 56 : 24, containLabel: true },
      legend:
        mode === "perChannel"
          ? { type: "scroll", bottom: 0, textStyle: { color: chart.label, fontSize: 11 } }
          : undefined,
      xAxis: {
        type: "time",
        axisLine: { lineStyle: { color: chart.axis } },
        axisLabel: { color: chart.label, fontSize: 10, hideOverlap: true },
        splitLine: { show: false },
      },
      yAxis: {
        type: "value",
        scale: mode === "perChannel",
        axisLabel: { color: chart.label, fontSize: 10 },
        splitLine: { lineStyle: { color: chart.split } },
      },
      tooltip: {
        trigger: "axis",
        axisPointer: { type: "cross", label: { show: false } },
        backgroundColor: chart.tooltipBg,
        borderColor: chart.tooltipBorder,
        textStyle: { color: chart.tooltipText, fontSize: 12 },
        formatter: (params: unknown) => {
          const list = params as { value: [number, number]; seriesName: string; color: string }[];
          if (!list.length) return "";

          const header = formatDateTime(new Date(list[0].value[0]));
          const lines = list.map((item) => {
            const dot = `<span style="display:inline-block;width:8px;height:8px;border-radius:50%;background:${item.color};margin-right:6px"></span>`;
            return `${dot}${item.seriesName}: <b>${formatPoints(item.value[1])}</b>`;
          });

          // Annotate with the reason when a single point lines up exactly.
          const at = list[0].value[0];
          const match = visible.find((r) => parseNaiveLocal(r.point.created_at).getTime() === at);
          if (match) {
            const reason = describeReason(match.point.points_info, match.prediction?.title ?? null);
            const delta = match.difference == null ? "" : ` (${formatDelta(match.difference)})`;
            lines.push(`<span style="opacity:.7">${reason}${delta}</span>`);
          }

          return `${header}<br/>${lines.join("<br/>")}`;
        },
      },
      dataZoom: [
        { type: "inside" },
        ...(showSlider ? [{ type: "slider" as const, height: 24, bottom: 24 }] : []),
      ],
      series,
    };
  }, [rows, mode, names, since, chart, showSlider]);

  return <EChart option={option} height={height} replaceMerge={["series"]} />;
}
