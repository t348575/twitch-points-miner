# twitch-points-miner ![Visits](https://nkvnu62257.execute-api.ap-south-1.amazonaws.com/production?repo=twitch-points-miner)

A very lightweight twitch points miner, using only a few MB of ram, inspired by [Twitch-Channel-Points-Miner-v2](https://github.com/rdavydov/Twitch-Channel-Points-Miner-v2).

![Landing page](assets/tpm-ui-landing.png "Web UI")

## Features
* Web UI to interact with the app, and change configurations at runtime [screenshots](#Web-UI-screenshots)
* Auto place bets on predictions
* Watch stream to collect view points
* Claim view point bonuses
* Follow raids
* REST API to manage app (Swagger docs at /docs)
* Analytics logging all actions

## Configuration
Check [example.config.yaml](example.config.yaml) for an example configuration.

Use the log level `info` for adequate information. Use `debug` for detailed logs, or if you feel a bug is present.

## Docker image
This is the suggested way of using twitch-points-miner.

Pull [t348575/twitch-points-miner](https://hub.docker.com/r/t348575/twitch-points-miner), be sure to pass your config file, and a volume for your `tokens.json`, as well as appropriate CLI arguments.

Run with stdin attached the first time, in order to authenticate your twitch account. Place your `config.yaml` file in the `data` dir.
```
docker run -i -t -v ./data:/data t348575/twitch-points-miner --token /data/tokens.json
```
Once it is running and the login flow is complete, CTRL+C then just attach the tokens file in subsequent runs

**Note**: The image comes with the `web_api` and `analytics` feature enabled.
**Note**: Don't forget to add /analytics.db as a docker volume, or specify the analytics database path in the config file.

## Docker compose
**Note**: You might want to touch all of the attached files first, or docker might create them as directories.
```yaml
services:
  twitch-points-miner:
    container_name: twitch-points-miner
    image: t348575/twitch-points-miner:latest
    volumes:
      - ./tokens.json:/tokens.json
      - ./config.yaml:/config.yaml
      - ./analytics.db:/analytics.db
      - ./twitch-points-miner.log:/twitch-points-miner.log
    command:
      - --log-to-file
    ports:
      - 3000:3000
    environment:
      - LOG=info
```

## Windows
Has not been tested on windows, but should work fine

## Building
```
cargo build --release
```

* Build with the feature `web_api` to enable the REST API server for management, and with the `analytics` feature to enable storing points and prediction information in an sqlite database.
```
cargo build --release --features web_api,analytics
```

## Web UI screenshots
![Landing page](assets/tpm-ui-landing.png "Web UI")
![Place predictions](assets/tpm-ui-make-prediction.png "Place predictions manually")
![Setup page](assets/tpm-ui-setup.png "Setup page")
![Configuration editor](assets/tpm-ui-edit-config.png "Configuration editor")