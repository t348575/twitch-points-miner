/**
 * Domain types for the twitch-points-miner REST API.
 *
 * These are hand written on purpose. `npm run api:types` can regenerate
 * `schema.d.ts` from the backend's OpenAPI document for drift checking, but the
 * generated output is wrong in three places and cannot be used directly:
 *
 *  1. `StreamerState.predictions` generates as `(Event & boolean)[]`. The real
 *     JSON is a map of `[Event, boolean]` tuples (common/src/types.rs).
 *  2. `GET /api/config/presets` is annotated `body = [HashMap<..>]` so it
 *     generates as an array of maps. It is a single object.
 *  3. `Outcome` collides between the analytics model and the twitch pubsub
 *     model; only the analytics one survives.
 */

/** Betting amount. `max_value` of 0 means "no cap". */
export interface Points {
  max_value: number;
  percent: number;
}

export type OddsComparisonType = "Le" | "Ge";

export interface DetailedOdds {
  _type: OddsComparisonType;
  threshold: number;
  attempt_rate: number;
  points: Points;
}

export interface DefaultPrediction {
  min_percentage: number;
  max_percentage: number;
  points: Points;
}

export interface Detailed {
  detailed: DetailedOdds[] | null;
  default: DefaultPrediction;
}

export interface Strategy {
  detailed: Detailed;
}

/** Serialized as an externally tagged enum, e.g. `{ "TotalUsers": 300 }`. */
export type Filter =
  | { TotalUsers: number }
  | { DelaySeconds: number }
  | { DelayPercentage: number };

export type FilterKind = "TotalUsers" | "DelaySeconds" | "DelayPercentage";

export interface PredictionConfig {
  strategy: Strategy;
  filters: Filter[];
}

export interface StreamerConfig {
  follow_raid: boolean;
  prediction: PredictionConfig;
}

/** Request shape when assigning a config to a streamer. */
export type ConfigType = { Preset: string } | { Specific: StreamerConfig };

/** Response shape. `"Specific"` is a bare string, a preset is an object. */
export type ConfigTypeRef = { Preset: string } | "Specific";

export interface StreamerConfigRefWrapper {
  _type: ConfigTypeRef;
  config: StreamerConfig;
}

export interface Game {
  id: string;
  name: string;
}

export interface StreamerInfo {
  broadcastId: string | null;
  live: boolean;
  channelName: string;
  game: Game | null;
}

/** An outcome of a prediction, as returned by both the live and analytics APIs. */
export interface Outcome {
  id: string;
  title: string;
  total_points: number;
  total_users: number;
}

/**
 * A live prediction from twitch pubsub.
 * `created_at` is RFC3339 *with* an offset - parse it with `new Date()`.
 */
export interface PredictionEvent {
  id: string;
  channel_id: string;
  title: string;
  status: string;
  created_at: string;
  ended_at: string | null;
  locked_at: string | null;
  outcomes: Outcome[];
  prediction_window_seconds: number;
  winning_outcome_id: string | null;
}

export interface StreamerState {
  info: StreamerInfo;
  /** Keyed by event id; the boolean is "we already placed a bet". */
  predictions: Record<string, [PredictionEvent, boolean]>;
  config: StreamerConfigRefWrapper;
  points: number;
}

export interface AppState {
  user_id: string;
  user_name: string;
  simulate: boolean;
  /** Keyed by the numeric channel id, as a string. */
  streamers: Record<string, StreamerState>;
  configs: Record<string, StreamerConfigRefWrapper>;
  watching: StreamerState[];
}

export interface LiveStreamer {
  id: number;
  state: StreamerState;
}

export type Presets = Record<string, StreamerConfig>;

/**
 * Why a points row was recorded. The `Prediction` variant carries
 * `[twitch event id, analytics row id]`.
 */
export type PointsInfo =
  | "FirstEntry"
  | "Watching"
  | "CommunityPointsClaimed"
  | { Prediction: [string, number] };

/**
 * A points balance snapshot. `points_value` is the absolute balance, not a delta.
 * `created_at` is naive local time with no offset - use `parseNaiveLocal`.
 */
export interface Point {
  channel_id: number;
  points_value: number;
  points_info: PointsInfo;
  created_at: string;
}

export type PredictionBetWrapper = "None" | { Some: { outcome_id: string; points: number } };

/** A settled or in-flight prediction from the analytics database. */
export interface AnalyticsPrediction {
  channel_id: number;
  prediction_id: string;
  title: string;
  prediction_window: number;
  outcomes: Outcome[];
  winning_outcome_id: string | null;
  placed_bet: PredictionBetWrapper;
  created_at: string;
  closed_at: string | null;
}

export interface TimelineResult {
  point: Point;
  /** SQL LAG delta against the previous row for this channel, within the query window. */
  difference: number | null;
  prediction: AnalyticsPrediction | null;
}

export interface TimelineRequest {
  from: string;
  to: string;
  channels: number[];
}

export interface MakePrediction {
  event_id: string;
  outcome_id: string;
  /** `null` means "let the configured strategy decide". */
  points: number | null;
}

/** Result of placing a bet: the backend answers 201 or 202. */
export type BetOutcome = "placed" | "declined";
