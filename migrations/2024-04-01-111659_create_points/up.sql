CREATE TABLE points (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    channel_id INTEGER NOT NULL,
    points_value INTEGER NOT NULL,
    points_info TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL,
    FOREIGN KEY (channel_id)
        REFERENCES streamers (id) 
);