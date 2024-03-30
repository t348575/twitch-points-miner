# twitch-points-miner ![Visits](https://nkvnu62257.execute-api.ap-south-1.amazonaws.com/production?repo=twitch-points-miner)

A lightweight twitch points miner, using only a few MB of ram, inspired by [Twitch-Channel-Points-Miner-v2](https://github.com/rdavydov/Twitch-Channel-Points-Miner-v2).

## Features
* Auto place bets on predictions
* Watch stream to collect view points
* Claim view point bonuses

## Configuration
Check [example.config.yaml](example.config.yaml) for an example configuration.

## Building
```
cargo build --release
```

* Build with the feature `web_api` to enable the REST API server for management.
```
cargo build --release --features web_api
```

## Docker image
* Image just 6.84 MB.
Build the image with the provided `Dockerfile` or just pull `t348575/twitch-points-miner`, be sure to pass your config file, and a volume for your `tokens.json`, as well as appropriate CLI arguments.

Run with stdin attached the first time, in order to authenticate your twitch account. Place your `config.yaml` file in the `data` dir.
```
docker run -i -t -v ./data:/data t348575/twitch-points-miner --token /data/tokens.json
```
Once it is running and the login flow is complete, CTRL+C then just attach the tokens file in subsequent runs

**Note**: The image comes with the `web_api` feature enabled.
