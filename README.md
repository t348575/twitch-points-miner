# twitch-points-miner ![Visits](https://nkvnu62257.execute-api.ap-south-1.amazonaws.com/production?repo=twitch-points-miner)

A twitch points miner inspired by [Twitch-Channel-Points-Miner-v2](https://github.com/rdavydov/Twitch-Channel-Points-Miner-v2), except its lightweight, the docker image is just 8.2 MB.

## Features
* Auto place bets on predictions
* Watch stream to collect watch view points
* Claim view point bonuses

## Configuration
Check [example.config.yaml](example.config.yaml) for an example configuration.

## Building
```
cargo build --release
```

* Build with the feature `api` to enable the REST API server for management.
```
cargo build --release --features api
```

## Docker image
Build the image with the provided `Dockerfile` or just pull `t348575/twitch-points-miner`, be sure to pass your config file, and a volume for your `tokens.json`, as well as appropriate CLI arguments.

Run with stdin attached the first time, in order to authenticate your twitch account.
```
docker run -i -t -v ./data:/data t348575/twitch-points-miner --token /data/tokens.json
```
Once it is running and the login flow is complete, CTRL+C then just attach the tokens file in subsequent runs

**Note**: The image comes with the `api` feature enabled.
