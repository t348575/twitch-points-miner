/**
 * This file was auto-generated by openapi-typescript.
 * Do not make direct changes to the file.
 */

export interface paths {
    "/api": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["app_state"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/bet/{streamer}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["make_prediction"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/live": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["live_streamers"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/live_prediction": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["get_live_prediction"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/timeline": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["points_timeline"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/{streamer}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["streamer"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
}
export type webhooks = Record<string, never>;
export interface components {
    schemas: {
        /** @enum {string} */
        ConfigTypeRef: "Preset" | "Specific";
        DefaultPrediction: {
            /** Format: double */
            max_percentage?: number;
            /** Format: double */
            min_percentage?: number;
            points: components["schemas"]["Points"];
        };
        Detailed: {
            default: components["schemas"]["DefaultPrediction"];
            high_odds?: components["schemas"]["HighOdds"][] | null;
        };
        /** @description Event */
        Event: {
            /** @description Channel ID */
            channel_id: string;
            created_at: components["schemas"]["Timestamp"];
            ended_at?: components["schemas"]["Timestamp"] | null;
            /** @description ID */
            id: string;
            locked_at?: components["schemas"]["Timestamp"] | null;
            /** @description Outcomes */
            outcomes: components["schemas"]["Outcome"][];
            /**
             * Format: int64
             * @description Prediction window in seconds
             */
            prediction_window_seconds: number;
            /** @description Status */
            status: string;
            /** @description Title */
            title: string;
            /** @description Winning outcome ID */
            winning_outcome_id?: string | null;
        };
        Filter: {
            /** Format: int32 */
            TotalUsers: number;
        } | {
            /** Format: int32 */
            DelaySeconds: number;
        } | {
            /** Format: double */
            DelayPercentage: number;
        };
        Game: {
            id: string;
            name: string;
        };
        HighOdds: {
            /** Format: double */
            high_odds_attempt_rate?: number;
            high_odds_points: components["schemas"]["Points"];
            /** Format: double */
            high_threshold?: number;
            /** Format: double */
            low_threshold?: number;
        };
        LiveStreamer: {
            /** Format: int32 */
            id: number;
            state: components["schemas"]["StreamerState"];
        };
        MakePrediction: {
            /** @description ID of the prediction */
            event_id: string;
            /** @description The outcome to place the bet on */
            outcome_id: string;
            /**
             * Format: int32
             * @description If specified, a bet is forcefully placed, otherwise the prediction logic specified in the configuration is used
             */
            points?: number | null;
        };
        Outcome: {
            id: string;
            title: string;
            /** Format: int64 */
            total_points: number;
            /** Format: int64 */
            total_users: number;
        };
        Outcomes: components["schemas"]["Outcome"][];
        Point: {
            /** Format: int32 */
            channel_id: number;
            /** Format: date-time */
            created_at: string;
            points_info: components["schemas"]["PointsInfo"];
            /** Format: int32 */
            points_value: number;
        };
        /** Format: int32 */
        Points: number;
        PointsInfo: "FirstEntry" | "Watching" | "CommunityPointsClaimed" | {
            /** @description prediction event id */
            Prediction: Record<string, never>[];
        };
        Prediction: {
            /** Format: int32 */
            channel_id: number;
            /** Format: date-time */
            closed_at?: string | null;
            /** Format: date-time */
            created_at: string;
            outcomes: components["schemas"]["Outcomes"];
            placed_bet: components["schemas"]["PredictionBetWrapper"];
            prediction_id: string;
            /** Format: int64 */
            prediction_window: number;
            title: string;
            winning_outcome_id?: string | null;
        };
        PredictionBet: {
            outcome_id: string;
            /** Format: int32 */
            points: number;
        };
        PredictionBetWrapper: "None" | {
            Some: components["schemas"]["PredictionBet"];
        };
        PubSub: {
            configs: {
                [key: string]: components["schemas"]["StreamerConfigRefWrapper"] | undefined;
            };
            simulate: boolean;
            streamers: {
                [key: string]: components["schemas"]["StreamerState"] | undefined;
            };
            user_id: string;
            user_name: string;
        };
        Strategy: {
            detailed: components["schemas"]["Detailed"];
        };
        StreamerConfig: {
            filters: components["schemas"]["Filter"][];
            strategy: components["schemas"]["Strategy"];
        };
        StreamerConfigRefWrapper: {
            _type: components["schemas"]["ConfigTypeRef"];
            config: components["schemas"]["StreamerConfig"];
        };
        StreamerInfo: {
            broadcastId?: components["schemas"]["UserId"] | null;
            channelName: string;
            game?: components["schemas"]["Game"] | null;
            live: boolean;
        };
        StreamerState: {
            config: components["schemas"]["StreamerConfigRefWrapper"];
            info: components["schemas"]["StreamerInfo"];
            /** Format: int32 */
            points: number;
            predictions: {
                [key: string]: (components["schemas"]["Event"] & boolean)[] | undefined;
            };
        };
        /** @description Timeline information, RFC3339 strings */
        Timeline: {
            /** @description Channels */
            channels: number[];
            /** @description GE time */
            from: string;
            /** @description LE time */
            to: string;
        };
        TimelineResult: {
            point: components["schemas"]["Point"];
            prediction?: components["schemas"]["Prediction"] | null;
        };
        /** @description RFC3339 timestamp */
        Timestamp: string;
        UserId: string;
    };
    responses: never;
    parameters: never;
    requestBodies: never;
    headers: never;
    pathItems: never;
}
export type $defs = Record<string, never>;
export interface operations {
    app_state: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Get the entire application state information */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PubSub"];
                };
            };
        };
    };
    make_prediction: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Name of streamer to get state for */
                streamer: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["MakePrediction"];
            };
        };
        responses: {
            /** @description Placed a bet */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Points"];
                };
            };
            /** @description Did not place a bet, but no error occurred */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Could not find streamer or event ID */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    live_streamers: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description List of live streamers and their state */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["LiveStreamer"][];
                };
            };
        };
    };
    get_live_prediction: {
        parameters: {
            query: {
                prediction_id: string;
                channel_id: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Get live prediction */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Prediction"] | null;
                };
            };
        };
    };
    points_timeline: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["Timeline"];
            };
        };
        responses: {
            /** @description Timeline of point information in the specified range */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TimelineResult"][];
                };
            };
        };
    };
    streamer: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Name of streamer to get state for */
                streamer: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Get the entire application state information */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["StreamerState"][];
                };
            };
            /** @description Could not find streamer */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
}
