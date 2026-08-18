<h1 align="center">twitch-points-miner</h1>
<p align="center">
  <img alt="Views" src="https://lambda.348575.xyz/repo-view-counter?repo=twitch-points-miner"/>
  <img alt="Build status" src="https://github.com/t348575/twitch-points-miner/actions/workflows/rust.yml/badge.svg"/>
  <img alt="Docker Image Version" src="https://img.shields.io/docker/v/t348575/twitch-points-miner"/>
  <img alt="Docker Pulls" src="https://img.shields.io/docker/pulls/t348575/twitch-points-miner"/>
  <img alt="Docker Image Size" src="https://img.shields.io/docker/image-size/t348575/twitch-points-miner"/>
</p>

<p align="center">
  A very lightweight twitch points miner, using only a few MB of ram, inspired by <a href="https://github.com/rdavydov/Twitch-Channel-Points-Miner-v2">Twitch-Channel-Points-Miner-v2</a>.
</p>

![Landing page](assets/tpm-ui-landing.png "Web UI")

## Features

- Web UI to interact with the app, and change configurations at runtime [screenshots](#Web-UI-screenshots)
- Auto place bets on predictions
- Watch stream to collect view points
- Claim view point bonuses
- Follow raids
- REST API to manage app (Swagger docs at /docs)
- Analytics logging all actions

## Behavior Details

### Watch Priority

The `watch_priority` list determines which streams are "watched" for viewership points when more than two configured streamers are live. The application can watch a maximum of two streams at once. The list is ordered from **highest priority (first item) to lowest priority (last item)**.

### Point Claiming

The application periodically checks for and claims community point bonuses (the clickable chests) for **every configured streamer that is currently live**. This action is independent of the `watch_priority` list. This is why you will see it successfully claim bonuses from channels that are not one of the two being actively "watched" for viewership points.

### Joining Raids

The application listens for "go live" notifications for **every streamer** in your configuration. When any of these streamers go live, the application then subscribes to their specific raid notifications. This allows it to follow a raid (if `follow_raid: true` is enabled for that streamer) even if the channel is not one of the two being actively "watched".

## Configuration

Check [example.config.yaml](example.config.yaml) for an example configuration.

For a complete list of all configuration possibilities, check [common/src/config](common/src/config).

Use the log level `info` for adequate information. Use `debug` for detailed logs, or if you feel a bug is present.

## Docker image

This is the suggested way of using twitch-points-miner.

Pull [t348575/twitch-points-miner](https://hub.docker.com/r/t348575/twitch-points-miner), be sure to pass your config file, and a volume for your `tokens.json`, as well as appropriate CLI arguments.

Run with stdin attached the first time, in order to authenticate your twitch account.

```
docker run -i -t -v ./data:/data t348575/twitch-points-miner --token /data/tokens.json
```

Once it is running and the login flow is complete, CTRL+C then just attach the tokens file in subsequent runs.

## Docker compose

An example docker compose file

```yaml
services:
  twitch-points-miner:
    container_name: twitch-points-miner
    image: t348575/twitch-points-miner:latest
    volumes:
      - ./data:/data
      - ./config.yaml:/config.yaml # change this if needed to your config file
    command:
      - -t
      - /data/tokens.json
      - --analytics-db
      - /data/analytics.db
      - --log-file
      - /data/twitch-points-miner.log
    ports:
      - 3000:3000 # Web UI port
    environment:
      - LOG=info
      - TZ=Europe/Vienna # Change to your specific timezone, see https://en.wikipedia.org/wiki/List_of_tz_database_time_zones#List
```

**Note on First Run:** The application requires an interactive terminal for the first-time login. When running for the first time, use the following command to attach your terminal and complete the authentication process:

```bash
docker-compose run --rm --service-ports twitch-points-miner
```

After the `tokens.json` file is created, you can stop the container (`CTRL+C`) and use `docker-compose up -d` for all subsequent runs.

### Authentication

The authentication process is designed to be interactive on the first run and automatic after that.

1.  **First Run:** When you start the application for the first time, it checks for the token file specified by the `-t` argument. Since it doesn't exist, it will begin an interactive login sequence in your terminal. You will be prompted to go to a Twitch URL in your browser and authorize the application. Confirm in the terminal

2.  **Token Creation:** After a successful login, the application saves your authentication credentials to the token file (e.g., `tokens.json`).

3.  **Subsequent Runs:** On all future runs, the application detects that the token file already exists. It reads the credentials from this file to authenticate with the Twitch API and starts immediately, skipping the interactive login.

### Command-Line Arguments

- `-t /data/tokens.json`: This is the short version of `--token`. It tells the application where to find or store the authentication `tokens.json` file.
- `--analytics-db /data/analytics.db`: This specifies the file path for the SQLite database where analytics data (like points history and prediction outcomes) is stored.
- `--log-file /data/twitch-points-miner.log`: This instructs the application to write its logs to the specified file, which is useful for debugging and reviewing historical activity.

## Windows

Has not been tested on windows, but should work fine

## Development

The easiest way to get a working dev setup is the included devcontainer, which comes with Rust, `diesel_cli`, and Node preinstalled.

### First-time setup

1. Open the repo in VS Code with the [Dev Containers](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers) extension installed.
2. Run **Dev Containers: Reopen in Container**. This builds the container and runs `.devcontainer/post-create.sh`, which installs system packages, updates the Rust toolchain, installs `diesel_cli`, installs UI dependencies, and does an initial `cargo build`.
3. Create a `config.yaml` in the repo root (see [example.config.yaml](example.config.yaml)).

### The `target/` directory

The devcontainer mounts cargo's `target/` as a named docker volume rather than
through the bind mount. Sharing it with the host causes ownership conflicts —
artifacts written by one user cannot be replaced by the other, and cargo fails
with `failed to create file ... Permission denied`. The volume also avoids the
bind-mount slowdown on the thousands of small files cargo writes.

`post-create.sh` chowns the volume on first creation, because docker creates
named volumes owned by root.

If you have a host-side `target/` left over from an earlier root-owned build, it
is now shadowed inside the container and harmless. To reclaim the disk space:

```bash
sudo rm -rf target
```

### Running the backend

In a devcontainer terminal:

```bash
cargo run -p twitch-points-miner -- -t data/tokens.json --analytics-db data/analytics.db --log-file data/twitch-points-miner.log
```

- If `data/tokens.json` does not exist yet, the app starts an interactive Twitch login: it prints a URL to open in your browser to authorize the app. Once confirmed, it writes `data/tokens.json` and continues running.
- To force a fresh login (e.g. an old token stopped working), move the existing token file out of the way first: `mv data/tokens.json data/tokens.json.bak`.
- On later runs, the existing `data/tokens.json` is reused automatically and no interactive step is needed.
- The devcontainer forwards port `3000`, so once running, the web UI is reachable at `http://localhost:3000` without any extra steps.

### Running the frontend dev server (optional)

Only needed if you're actively developing the frontend — the backend already serves the built UI on port `3000`. For live reload while editing UI code:

```bash
cd ui && npm run dev
```

The devcontainer also forwards port `5173` for this Vite dev server. `vite.config.ts` sets `server.host` so Vite binds to `0.0.0.0`; without it, Vite only binds to `localhost` inside the container, and VS Code's port forwarding can't reach it.

The dev server reads the backend URL from `ui/.env.development` (`VITE_API_BASE`, default `http://localhost:3000`). Production builds always call the origin they were served from.

To build the UI the way the backend serves it:

```bash
cd ui && npm run build   # writes to ../dist, which the backend serves
```

`dist/` is gitignored, so a fresh clone has nothing for the backend to serve
until this has run once. `post-create.sh` does it during devcontainer setup;
re-run it by hand after changing UI code if you are not using the dev server.

### Port 3000 vs port 5173

- **Port 3000** is the Rust backend. It serves the REST API, and in production also serves the built frontend files directly. Open this if you just want to use the app.
- **Port 5173** is the Vite dev server, used only when actively developing the frontend. It serves the UI with hot reload, but API calls still go to the backend (`VITE_API_BASE` in `ui/.env.development`). So both servers must be running at once when using port 5173: Vite for the live UI, and the Rust backend for the actual data.

### Frontend layout

The UI lives in `ui/` and is built with Vite, TypeScript, React, Mantine and ECharts.

Linting and formatting use [oxlint](https://oxc.rs/docs/guide/usage/linter) and
[oxfmt](https://oxc.rs/docs/guide/usage/formatter):

```bash
cd ui
npm run lint          # oxlint
npm run format        # oxfmt, rewrites files
npm run format:check  # oxfmt, fails if anything is unformatted
```

CI runs `lint` and `format:check` before the build.

The previous Svelte UI is still in `frontend/` for reference only. It is no longer built by Docker or CI; see `frontend/DEPRECATED.md`.

## Building

```
cargo build --release
cd ui && npm ci && npm run build
```

## Web UI screenshots

![Landing page](assets/tpm-ui-landing.png "Web UI")
![Place predictions](assets/tpm-ui-make-prediction.png "Place predictions manually")
![Setup page](assets/tpm-ui-setup.png "Setup page")
![Configuration editor](assets/tpm-ui-edit-config.png "Configuration editor")
