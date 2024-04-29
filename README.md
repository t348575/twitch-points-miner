# twitch-points-miner
<p align="center">
<img alt="Views" src="https://xn4nc029ta.execute-api.ap-south-1.amazonaws.com/default/repo-view-counter?repo=twitch-points-miner">
<img alt="Docker Image Version" src="https://img.shields.io/docker/v/t348575/twitch-points-miner">
<img alt="Docker Pulls" src="https://img.shields.io/docker/pulls/t348575/twitch-points-miner">
<img alt="Docker Image Size" src="https://img.shields.io/docker/image-size/t348575/twitch-points-miner">
</p>


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

For a complete list of all configuration possibilities, check [common/src/config](common/src/config).

Use the log level `info` for adequate information. Use `debug` for detailed logs, or if you feel a bug is present.

## Docker image
This is the suggested way of using twitch-points-miner.

Pull [t348575/twitch-points-miner](https://hub.docker.com/r/t348575/twitch-points-miner), be sure to pass your config file, and a volume for your `tokens.json`, as well as appropriate CLI arguments.

Run with stdin attached the first time, in order to authenticate your twitch account. Place your `config.yaml` file in the `data` dir.
```
docker run -i -t -v ./data:/data t348575/twitch-points-miner --token /data/tokens.json
```
Once it is running and the login flow is complete, CTRL+C then just attach the tokens file in subsequent runs

**Note**: Don't forget to add /analytics.db as a docker volume, or specify the analytics database path in the config file.

## Docker compose
**Note**: You might want to touch all of the attached files first, or docker might create them as directories.
```yaml
services:
  twitch-points-miner:
    container_name: twitch-points-miner
    image: t348575/twitch-points-miner:latest
    volumes:
      - ./data/tokens.json:/tokens.json
      - ./config.yaml:/config.yaml
      - ./analytics.db:/analytics.db
      - ./twitch-points-miner.log:/twitch-points-miner.log
    command:
      - --log-to-file
    ports:
      - 3000:3000 # Web UI port
    environment:
      - LOG=info
```

## Windows
Has not been tested on windows, but should work fine

## Building
```
cargo build --release
```

## Web UI screenshots
![Landing page](assets/tpm-ui-landing.png "Web UI")
![Place predictions](assets/tpm-ui-make-prediction.png "Place predictions manually")
![Setup page](assets/tpm-ui-setup.png "Setup page")
![Configuration editor](assets/tpm-ui-edit-config.png "Configuration editor")