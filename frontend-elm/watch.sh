#!/usr/bin/bash

cargo tauri dev &

inotifywait -m -r -e modify,delete,create src | while read; do
    elm make src/Main.elm --output=public/main.js;
done