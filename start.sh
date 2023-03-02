#!/bin/bash

./server/cootsmania --server localhost:1155 &
./caddy/caddy run &

wait