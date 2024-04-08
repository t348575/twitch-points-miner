CREATE TABLE predictions (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    channel_id INTEGER NOT NULL,
    prediction_id TEXT NOT NULL,
    title TEXT NOT NULL,
    prediction_window BIGINT NOT NULL,
    outcomes TEXT NOT NULL,
    winning_outcome_id TEXT,
    placed_bet TEXT,
    created_at TIMESTAMP NOT NULL,
    closed_at TIMESTAMP,
    FOREIGN KEY (channel_id)
        REFERENCES streamers (id) 
)