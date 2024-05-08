# v0.1.11
* Fix various UI bugs on prediction page
* Fix runtime blocking bug

# v0.1.10
* Removed `analytics_db` from config file to argument `analytics_db`
* Rename `log_to_file` argument to `log_file` and accepts path to the log file
* Add websocket pooling, increasing maximum streamers to around 250 (suggested, not definite)