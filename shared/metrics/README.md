# Usage
Specific to Linux.

## Build and run game with metrics feature
```cargo run --features metrics```

## Run prometheus server
```
# install prometheus
# set GAME_DIR env var to cloned repository root

$ ls -l $GAME_DIR
Cargo.lock  data  LICENSE      README.md  resources  target
...

# run prometheus from its web dir (e.g. /usr/share/prometheus/web/ui)
$ ls
static  templates

$ prometheus --config.file=$GAME_DIR/shared/metrics/prometheus.yml --storage.tsdb.path=/tmp/prometheus --web.listen-address="127.0.0.1:9090"
```

### Run grafana
* Install grafana
* Browse to `localhost:3000` and login
* Configuration > Data Sources > Add data source
* Choose "Prometheus"
* Configure:
    * URL: `http://localhost:9090`
        * Necessary step, even though it seems to be auto-populated
    * Scrape interval: `1s`
* Now add a dashboard