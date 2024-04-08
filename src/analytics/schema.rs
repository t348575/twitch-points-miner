// @generated automatically by Diesel CLI.

diesel::table! {
    points (id) {
        id -> Integer,
        channel_id -> Integer,
        points_value -> Integer,
        points_info -> Text,
        created_at -> Timestamp,
    }
}

diesel::table! {
    predictions (id) {
        id -> Integer,
        channel_id -> Integer,
        prediction_id -> Text,
        title -> Text,
        prediction_window -> BigInt,
        outcomes -> Text,
        winning_outcome_id -> Nullable<Text>,
        placed_bet -> Nullable<Text>,
        created_at -> Timestamp,
        closed_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    streamers (id) {
        id -> Integer,
        name -> Text,
    }
}

diesel::joinable!(points -> streamers (channel_id));
diesel::joinable!(predictions -> streamers (channel_id));

diesel::allow_tables_to_appear_in_same_query!(points, predictions, streamers,);
